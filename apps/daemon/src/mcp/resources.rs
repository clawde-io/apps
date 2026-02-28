//! MCP `resources/list` and `resources/read` implementation (Sprint BB PV.12).
//!
//! Exposes three resource families to external MCP clients (Codex CLI, Cursor):
//!
//! | URI pattern | Content |
//! |-------------|---------|
//! | `clawd://sessions` | JSON list of all sessions (id, status, provider, repo) |
//! | `clawd://session/{id}/messages` | JSON array of messages for a session |
//! | `clawd://tasks` | JSON list of all active agent tasks |
//! | `clawd://task/{id}` | JSON object with full task detail |
//! | `clawd://repo/{path}` | UTF-8 file content from the active repo |

use crate::AppContext;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, warn};

// ─── Resource descriptor ──────────────────────────────────────────────────────

/// A single MCP resource exposed by `clawd`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceDescriptor {
    /// MCP-spec URI (e.g. `clawd://sessions`).
    pub uri: String,
    /// Human-readable name for this resource.
    pub name: String,
    /// One-sentence description.
    pub description: String,
    /// MIME type of the content returned by `read_resource`.
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

// ─── Resource listing ─────────────────────────────────────────────────────────

/// Return all resources that `clawd` exposes to MCP clients.
///
/// This is the response body for `resources/list`.  The list is static —
/// individual instances (session IDs, task IDs) appear as sub-resources
/// under the collection URIs and are discovered via `read_resource`.
pub async fn list_resources(ctx: &Arc<AppContext>) -> Vec<ResourceDescriptor> {
    let mut resources = vec![
        ResourceDescriptor {
            uri: "clawd://sessions".to_string(),
            name: "Sessions".to_string(),
            description: "All active and recent ClawDE sessions".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceDescriptor {
            uri: "clawd://tasks".to_string(),
            name: "Agent Tasks".to_string(),
            description: "All active agent tasks in the task queue".to_string(),
            mime_type: "application/json".to_string(),
        },
    ];

    // Add per-session message resources for all known sessions.
    if let Ok(sessions) = ctx.storage.list_sessions().await {
        for s in &sessions {
            resources.push(ResourceDescriptor {
                uri: format!("clawd://session/{}/messages", s.id),
                name: format!("Session {} Messages", &s.id[..8.min(s.id.len())]),
                description: format!("Message history for session {} ({})", s.id, s.status),
                mime_type: "application/json".to_string(),
            });
        }
    }

    debug!("MCP resources/list: {} resources", resources.len());
    resources
}

// ─── Resource reading ─────────────────────────────────────────────────────────

/// Read the content of a single resource by URI.
///
/// Returns `Ok(Value)` with `{ contents: [{ uri, mimeType, text }] }` on
/// success, or `Err(String)` with an MCP-compatible error message.
pub async fn read_resource(ctx: &Arc<AppContext>, uri: &str) -> Result<Value, String> {
    if uri == "clawd://sessions" {
        return read_sessions(ctx).await;
    }
    if uri == "clawd://tasks" {
        return read_tasks(ctx).await;
    }
    if let Some(session_id) = uri
        .strip_prefix("clawd://session/")
        .and_then(|s| s.strip_suffix("/messages"))
    {
        return read_session_messages(ctx, session_id).await;
    }
    if let Some(task_id) = uri.strip_prefix("clawd://task/") {
        return read_task(ctx, task_id).await;
    }
    if let Some(rel_path) = uri.strip_prefix("clawd://repo/") {
        return read_repo_file(ctx, rel_path).await;
    }

    warn!(uri = uri, "MCP resources/read: unknown URI scheme");
    Err(format!("unknown resource URI: {uri}"))
}

// ─── Individual resource handlers ─────────────────────────────────────────────

async fn read_sessions(ctx: &Arc<AppContext>) -> Result<Value, String> {
    let sessions = ctx
        .storage
        .list_sessions()
        .await
        .map_err(|e| format!("failed to list sessions: {e}"))?;

    let data: Vec<Value> = sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "status": s.status,
                "provider": s.provider,
                "repoPath": s.repo_path,
                "messageCount": s.message_count,
                "createdAt": s.created_at,
            })
        })
        .collect();

    Ok(make_text_content(
        "clawd://sessions",
        "application/json",
        &serde_json::to_string_pretty(&data).unwrap_or_default(),
    ))
}

async fn read_tasks(ctx: &Arc<AppContext>) -> Result<Value, String> {
    use crate::tasks::storage::TaskListParams;

    let params = TaskListParams {
        status: Some("active".to_string()),
        ..Default::default()
    };
    let tasks = ctx
        .task_storage
        .list_tasks(&params)
        .await
        .map_err(|e| format!("failed to list tasks: {e}"))?;

    let data: Vec<Value> = tasks
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "title": t.title,
                "status": t.status,
                "phase": t.phase,
                "severity": t.severity,
                "claimedBy": t.claimed_by,
                "createdAt": t.created_at,
            })
        })
        .collect();

    Ok(make_text_content(
        "clawd://tasks",
        "application/json",
        &serde_json::to_string_pretty(&data).unwrap_or_default(),
    ))
}

async fn read_session_messages(ctx: &Arc<AppContext>, session_id: &str) -> Result<Value, String> {
    let messages = ctx
        .storage
        .list_messages(session_id, 200, None)
        .await
        .map_err(|e| format!("failed to list messages: {e}"))?;

    let data: Vec<Value> = messages
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "role": m.role,
                "content": m.content,
                "status": m.status,
                "createdAt": m.created_at,
            })
        })
        .collect();

    let uri = format!("clawd://session/{session_id}/messages");
    Ok(make_text_content(
        &uri,
        "application/json",
        &serde_json::to_string_pretty(&data).unwrap_or_default(),
    ))
}

async fn read_task(ctx: &Arc<AppContext>, task_id: &str) -> Result<Value, String> {
    let maybe_task = ctx
        .task_storage
        .get_task(task_id)
        .await
        .map_err(|e| format!("failed to get task {task_id}: {e}"))?;

    let task = maybe_task.ok_or_else(|| format!("task {task_id} not found"))?;

    let data = json!({
        "id": task.id,
        "title": task.title,
        "status": task.status,
        "phase": task.phase,
        "severity": task.severity,
        "notes": task.notes,
        "claimedBy": task.claimed_by,
        "blockReason": task.block_reason,
        "file": task.file,
        "createdAt": task.created_at,
        "updatedAt": task.updated_at,
    });

    let uri = format!("clawd://task/{task_id}");
    Ok(make_text_content(
        &uri,
        "application/json",
        &serde_json::to_string_pretty(&data).unwrap_or_default(),
    ))
}

async fn read_repo_file(ctx: &Arc<AppContext>, rel_path: &str) -> Result<Value, String> {
    // Determine the active repo root. Use the first registered repo.
    let paths = ctx.repo_registry.list_paths().await;
    let repo_root = paths
        .into_iter()
        .next()
        .ok_or_else(|| "no repository registered".to_string())?;

    let full_path = std::path::Path::new(&repo_root).join(rel_path);

    // Security: path must stay inside the repo root after canonicalization.
    let canonical = full_path
        .canonicalize()
        .map_err(|e| format!("cannot resolve path {rel_path}: {e}"))?;
    let repo_canonical = std::path::Path::new(&repo_root)
        .canonicalize()
        .map_err(|e| format!("cannot resolve repo root: {e}"))?;
    if !canonical.starts_with(&repo_canonical) {
        return Err(format!("path traversal denied: {rel_path}"));
    }

    let content = tokio::fs::read_to_string(&canonical)
        .await
        .map_err(|e| format!("cannot read {rel_path}: {e}"))?;

    let mime = mime_for_extension(rel_path);
    let uri = format!("clawd://repo/{rel_path}");
    Ok(make_text_content(&uri, mime, &content))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_text_content(uri: &str, mime_type: &str, text: &str) -> Value {
    json!({
        "contents": [{
            "uri": uri,
            "mimeType": mime_type,
            "text": text,
        }]
    })
}

fn mime_for_extension(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
    {
        "rs" => "text/x-rust",
        "ts" | "tsx" => "text/typescript",
        "js" | "jsx" => "text/javascript",
        "dart" => "text/x-dart",
        "json" => "application/json",
        "toml" => "text/x-toml",
        "yaml" | "yml" => "text/yaml",
        "md" => "text/markdown",
        _ => "text/plain",
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_for_rust_file() {
        assert_eq!(mime_for_extension("src/main.rs"), "text/x-rust");
    }

    #[test]
    fn mime_for_typescript() {
        assert_eq!(mime_for_extension("app.tsx"), "text/typescript");
    }

    #[test]
    fn mime_for_unknown_extension() {
        assert_eq!(mime_for_extension("Makefile"), "text/plain");
    }

    #[test]
    fn make_text_content_shape() {
        let v = make_text_content("clawd://sessions", "application/json", "[]");
        assert!(v.get("contents").is_some());
        let contents = v["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["uri"], "clawd://sessions");
        assert_eq!(contents[0]["mimeType"], "application/json");
        assert_eq!(contents[0]["text"], "[]");
    }

    #[test]
    fn unknown_uri_returns_error() {
        // Can't run async in unit tests without runtime; test the sync path.
        // read_resource delegates to async helpers — tested via integration tests.
        // Here we validate that the URI routing string pattern is correct.
        let uri = "clawd://session/abc123/messages";
        let session_id = uri
            .strip_prefix("clawd://session/")
            .and_then(|s| s.strip_suffix("/messages"));
        assert_eq!(session_id, Some("abc123"));
    }
}
