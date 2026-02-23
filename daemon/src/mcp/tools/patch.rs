/// MCP `apply_patch` tool handler.
///
/// Validates and applies a unified-diff patch to the task's worktree.
/// Idempotent: a given `idempotency_key` is only applied once; subsequent
/// calls with the same key return the cached result without re-applying.
///
/// Patch validation uses `git2::Diff::from_buffer` to verify the diff is
/// well-formed before touching the filesystem.  Actual application is done
/// via `git2::Repository::apply` when the diff is clean.
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::OnceLock;

// ─── Idempotency key store ────────────────────────────────────────────────────

/// In-memory store of seen idempotency keys.
///
/// Using a global `OnceLock<Mutex<HashSet>>` keeps the implementation simple
/// and avoids threading an extra store through `AppContext`.  For high-volume
/// use cases this could be moved to the SQLite activity log.
static SEEN_KEYS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn seen_keys() -> &'static Mutex<HashSet<String>> {
    SEEN_KEYS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Check whether a key has been seen before.  If not, records it and returns
/// `false`; if already seen, returns `true` (idempotent — skip re-application).
fn check_and_record_key(key: &str) -> bool {
    let mut guard = seen_keys()
        .lock()
        .expect("idempotency key store poisoned");
    if guard.contains(key) {
        true // already seen
    } else {
        guard.insert(key.to_string());
        false // first time
    }
}

// ─── apply_patch ─────────────────────────────────────────────────────────────

/// MCP `apply_patch` handler.
///
/// Required: `task_id`, `patch` (unified diff string), `idempotency_key`.
///
/// Returns `{"applied": bool, "files_changed": [String]}`.
pub async fn apply_patch(ctx: &AppContext, args: Value, agent_id: Option<&str>) -> Result<Value> {
    let task_id = args
        .get("task_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("MCP_INVALID_PARAMS: missing required field 'task_id'"))?;

    let patch_str = args
        .get("patch")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("MCP_INVALID_PARAMS: missing required field 'patch'"))?;

    let idempotency_key = args
        .get("idempotency_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("MCP_INVALID_PARAMS: missing required field 'idempotency_key'")
        })?;

    let aid = agent_id.unwrap_or("mcp-agent");

    // Look up the task to get the repo path.
    let task = ctx
        .task_storage
        .get_task(task_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("MCP_INVALID_PARAMS: task '{}' not found", task_id))?;

    // Verify task is Active+Claimed (belt-and-suspenders; dispatcher also checks).
    if task.status != "in_progress" {
        return Err(anyhow::anyhow!(
            "MCP_PROVIDER_NOT_AVAILABLE: task '{}' is '{}' — must be 'in_progress'",
            task_id,
            task.status
        ));
    }

    // Idempotency check — if we have already applied this key, return early.
    if check_and_record_key(idempotency_key) {
        tracing::debug!(
            key = %idempotency_key,
            "apply_patch: idempotency key already seen — skipping re-application"
        );
        return Ok(json!({
            "applied": false,
            "files_changed": [],
            "idempotent_skip": true
        }));
    }

    // Validate the patch is well-formed using git2.
    let patch_bytes = patch_str.as_bytes();
    let diff =
        git2::Diff::from_buffer(patch_bytes).map_err(|e| {
            anyhow::anyhow!("MCP_INVALID_PARAMS: invalid patch: {}", e)
        })?;

    // Extract the list of files touched by the patch.
    let mut files_changed: Vec<String> = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            if let Some(path) = delta.new_file().path() {
                files_changed.push(path.to_string_lossy().into_owned());
            } else if let Some(path) = delta.old_file().path() {
                files_changed.push(path.to_string_lossy().into_owned());
            }
            true
        },
        None,
        None,
        None,
    )
    .map_err(|e| anyhow::anyhow!("patch delta enumeration failed: {}", e))?;

    // Open the task's isolated git worktree (NOT the project root).
    // Each code-modifying task is bound to a dedicated worktree so that
    // concurrent tasks cannot interfere with each other's working trees.
    let worktree_info = ctx
        .worktree_manager
        .get(task_id)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "MCP_INVALID_PARAMS: task '{}' has no worktree — claim the task first to provision one",
                task_id
            )
        })?;
    let repo_path = &worktree_info.worktree_path;
    let repo = git2::Repository::open(repo_path)
        .map_err(|e| anyhow::anyhow!("failed to open worktree at '{}': {}", repo_path.display(), e))?;

    let mut apply_opts = git2::ApplyOptions::new();
    // Apply to workdir (index=false), so the changes appear as working-tree edits.
    repo.apply(&diff, git2::ApplyLocation::WorkDir, Some(&mut apply_opts))
        .map_err(|e| anyhow::anyhow!("git apply failed: {}", e))?;

    // Log the operation.
    let detail = format!(
        "apply_patch: files_changed={} idempotency_key={}",
        files_changed.len(),
        idempotency_key
    );
    let _ = ctx
        .task_storage
        .log_activity(
            aid,
            Some(task_id),
            task.phase.as_deref(),
            "apply_patch",
            "system",
            Some(&detail),
            None,
            &task.repo_path,
        )
        .await;

    tracing::info!(
        task_id = %task_id,
        files = %files_changed.len(),
        key = %idempotency_key,
        "MCP apply_patch applied"
    );

    Ok(json!({
        "applied": true,
        "files_changed": files_changed
    }))
}
