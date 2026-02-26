//! Sprint CC GD.3/GD.4 — `ghost_diff.*` RPC handlers.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `ghost_diff.check` — run ghost diff on session's changed files vs specs.
///
/// Broadcasts `ghost_diff.driftDetected` (GD.4) when drift warnings are found,
/// so the Flutter UI can show a live indicator without polling.
pub async fn check(params: Value, ctx: AppContext) -> Result<Value> {
    let repo_path = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let session_id = params.get("sessionId").and_then(|v| v.as_str());

    let warnings =
        crate::ghost_diff::engine::check_ghost_drift(repo_path, session_id).await?;

    let has_drift = !warnings.is_empty();
    let warnings_json: Vec<Value> = warnings
        .iter()
        .map(|w| {
            json!({
                "file": w.file,
                "spec": w.spec,
                "divergenceSummary": w.divergence_summary,
                "severity": w.severity,
            })
        })
        .collect();

    // GD.4: push live notification when drift is detected.
    if has_drift {
        ctx.broadcaster.broadcast(
            "ghost_diff.driftDetected",
            json!({
                "repoPath": repo_path,
                "sessionId": session_id,
                "warningCount": warnings_json.len(),
                "warnings": &warnings_json,
            }),
        );
    }

    Ok(json!({
        "repoPath": repo_path,
        "sessionId": session_id,
        "warnings": warnings_json,
        "hasDrift": has_drift,
    }))
}
