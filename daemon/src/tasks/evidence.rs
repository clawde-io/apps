// tasks/evidence.rs — Evidence pack builder (Sprint ZZ EP.T02)
//
// At task completion, assembles: git diff stats, test results, tool call trace,
// review verdict → stores as an evidence_pack row.

use crate::storage::Storage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Summary of git diff statistics from a worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub files: Vec<String>,
}

/// Result from a test run (from CI runner or verify loop).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub duration_ms: u64,
    pub first_failure: Option<String>,
}

/// Summarised tool call from a session window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEntry {
    pub tool: String,
    pub path: Option<String>,
    pub decision: String, // allow | deny | ask
    pub duration_ms: u64,
}

/// Assembled evidence pack for a completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidencePack {
    pub task_id: String,
    pub run_id: String,
    pub instruction_hash: String,
    pub policy_hash: String,
    pub worktree_commit: Option<String>,
    pub diff_stats: DiffStats,
    pub test_results: TestResults,
    pub tool_trace: Vec<ToolCallEntry>,
    pub reviewer_verdict: Option<String>, // pass | fail | needs_review
    pub created_at: i64,
}

/// Build an evidence pack by querying the storage for task-related data.
pub async fn build_evidence_pack(
    storage: &Storage,
    task_id: &str,
    run_id: &str,
    worktree_path: Option<&str>,
) -> Result<String> {
    let now = chrono::Utc::now().timestamp();

    // ── Diff stats from worktree ──────────────────────────────────────────────
    let diff_stats = if let Some(wt_path) = worktree_path {
        collect_diff_stats(wt_path).await.unwrap_or(DiffStats {
            files_changed: 0,
            insertions: 0,
            deletions: 0,
            files: vec![],
        })
    } else {
        DiffStats {
            files_changed: 0,
            insertions: 0,
            deletions: 0,
            files: vec![],
        }
    };

    // ── Test results from last CI run ─────────────────────────────────────────
    let test_results = query_test_results(storage, task_id).await?;

    // ── Tool call trace from audit events ────────────────────────────────────
    let tool_trace = query_tool_trace(storage, task_id).await?;

    // ── Review verdict from last review.run ──────────────────────────────────
    let reviewer_verdict = query_review_verdict(storage, task_id).await?;

    // ── Worktree HEAD commit ──────────────────────────────────────────────────
    let worktree_commit = if let Some(wt_path) = worktree_path {
        get_worktree_head_sha(wt_path).await
    } else {
        None
    };

    // ── Compute hashes ────────────────────────────────────────────────────────
    let instruction_hash = compute_instruction_hash(storage).await?;
    let policy_hash = compute_policy_hash(storage).await?;

    // ── Store in DB ───────────────────────────────────────────────────────────
    let pack_id = hex::encode(Sha256::digest(format!("{task_id}:{run_id}:{now}").as_bytes()));
    let pack_id = &pack_id[..32]; // 32-char prefix

    let diff_json = serde_json::to_string(&diff_stats)?;
    let test_json = serde_json::to_string(&test_results)?;
    let trace_json = serde_json::to_string(&tool_trace)?;

    sqlx::query(
        "INSERT INTO evidence_packs \
         (id, task_id, run_id, instruction_hash, policy_hash, worktree_commit, \
          diff_stats_json, test_results_json, tool_trace_json, reviewer_verdict, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(pack_id)
    .bind(task_id)
    .bind(run_id)
    .bind(&instruction_hash)
    .bind(&policy_hash)
    .bind(worktree_commit.as_deref())
    .bind(&diff_json)
    .bind(&test_json)
    .bind(&trace_json)
    .bind(reviewer_verdict.as_deref())
    .bind(now)
    .execute(storage.pool())
    .await?;

    Ok(pack_id.to_string())
}

/// Collect diff stats from a worktree using `git diff --stat HEAD`.
async fn collect_diff_stats(worktree_path: &str) -> Option<DiffStats> {
    let output = tokio::process::Command::new("git")
        .args(["-C", worktree_path, "diff", "--stat", "HEAD"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files_changed = 0u32;
    let mut insertions = 0u32;
    let mut deletions = 0u32;
    let mut files = Vec::new();

    for line in stdout.lines() {
        if line.contains(" | ") {
            // e.g. " src/foo.rs | 10 ++++"
            if let Some(fname) = line.split(" | ").next() {
                files.push(fname.trim().to_string());
                files_changed += 1;
            }
        } else if line.contains("changed") {
            // Summary line: "3 files changed, 42 insertions(+), 5 deletions(-)"
            for part in line.split(',') {
                let part = part.trim();
                if part.contains("insertion") {
                    if let Some(n) = part.split_whitespace().next().and_then(|s| s.parse().ok()) {
                        insertions = n;
                    }
                } else if part.contains("deletion") {
                    if let Some(n) = part.split_whitespace().next().and_then(|s| s.parse().ok()) {
                        deletions = n;
                    }
                }
            }
        }
    }

    Some(DiffStats {
        files_changed,
        insertions,
        deletions,
        files,
    })
}

/// Query the most recent test results for a task from CI run events.
async fn query_test_results(storage: &Storage, task_id: &str) -> Result<TestResults> {
    // Look for benchmark_runs linked to this task
    let row: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT \
            COALESCE(SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), 0), \
            0, \
            COALESCE(MAX(duration_ms), 0), \
            COUNT(*) \
         FROM benchmark_runs WHERE task_id = ?",
    )
    .bind(task_id)
    .fetch_optional(storage.pool())
    .await?;

    let (passed, failed, skipped, duration_ms, _total) =
        row.unwrap_or((0, 0, 0, 0, 0));

    Ok(TestResults {
        passed: passed as u32,
        failed: failed as u32,
        skipped: skipped as u32,
        duration_ms: duration_ms as u64,
        first_failure: None,
    })
}

/// Collect tool call trace from audit events within the task's time window.
async fn query_tool_trace(storage: &Storage, task_id: &str) -> Result<Vec<ToolCallEntry>> {
    // Query audit_log for tool-related events in this task's time window
    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT action, resource_id, metadata_json FROM audit_log \
         WHERE actor_id = ? AND action LIKE 'tool.%' \
         ORDER BY created_at ASC LIMIT 200",
    )
    .bind(task_id)
    .fetch_all(storage.pool())
    .await?;

    let entries: Vec<ToolCallEntry> = rows
        .into_iter()
        .map(|(action, resource_id, meta_json)| {
            let meta: serde_json::Value = meta_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(serde_json::Value::Null);

            let tool = action.trim_start_matches("tool.").to_string();
            let path = resource_id;
            let decision = meta["decision"]
                .as_str()
                .unwrap_or("allow")
                .to_string();
            let duration_ms = meta["duration_ms"].as_u64().unwrap_or(0);

            ToolCallEntry {
                tool,
                path,
                decision,
                duration_ms,
            }
        })
        .collect();

    Ok(entries)
}

/// Look for review verdict from the most recent review run for this task.
async fn query_review_verdict(storage: &Storage, task_id: &str) -> Result<Option<String>> {
    let verdict: Option<String> = sqlx::query_scalar(
        "SELECT reviewer_verdict FROM evidence_packs WHERE task_id = ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(storage.pool())
    .await?
    .flatten();

    Ok(verdict)
}

/// Get the HEAD SHA of a worktree.
async fn get_worktree_head_sha(worktree_path: &str) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(["-C", worktree_path, "rev-parse", "HEAD"])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Compute SHA-256 of the compiled CLAUDE.md (if it exists).
async fn compute_instruction_hash(storage: &Storage) -> Result<String> {
    let row: Option<String> = sqlx::query_scalar(
        "SELECT instruction_hash FROM instruction_compilations \
         WHERE target_format = 'claude' ORDER BY compiled_at DESC LIMIT 1",
    )
    .fetch_optional(storage.pool())
    .await?;

    Ok(row.unwrap_or_else(|| "none".to_string()))
}

/// Compute SHA-256 of policy configuration (stub: use instruction hash for now).
async fn compute_policy_hash(storage: &Storage) -> Result<String> {
    // Policy hash = SHA-256 of the combined instruction node IDs (as a proxy for policy state)
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM instruction_nodes ORDER BY id")
            .fetch_all(storage.pool())
            .await?;

    let combined = ids.join(",");
    let hash = hex::encode(Sha256::digest(combined.as_bytes()));
    Ok(hash[..16].to_string())
}

/// Retrieve an evidence pack by task_id.
pub async fn get_evidence_pack(storage: &Storage, task_id: &str) -> Result<Option<EvidencePack>> {
    let row: Option<(
        String, String, String, String, String,
        Option<String>, String, String, String, Option<String>, i64,
    )> = sqlx::query_as(
        "SELECT id, task_id, run_id, instruction_hash, policy_hash, \
         worktree_commit, diff_stats_json, test_results_json, tool_trace_json, \
         reviewer_verdict, created_at \
         FROM evidence_packs WHERE task_id = ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(storage.pool())
    .await?;

    if let Some((
        _id, task_id, run_id, instruction_hash, policy_hash,
        worktree_commit, diff_json, test_json, trace_json,
        reviewer_verdict, created_at,
    )) = row
    {
        let diff_stats: DiffStats = serde_json::from_str(&diff_json)?;
        let test_results: TestResults = serde_json::from_str(&test_json)?;
        let tool_trace: Vec<ToolCallEntry> = serde_json::from_str(&trace_json)?;

        Ok(Some(EvidencePack {
            task_id,
            run_id,
            instruction_hash,
            policy_hash,
            worktree_commit,
            diff_stats,
            test_results,
            tool_trace,
            reviewer_verdict,
            created_at,
        }))
    } else {
        Ok(None)
    }
}
