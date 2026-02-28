/// drift.scan and drift.list RPC handlers.
///
/// V02.T22: drift.scan — scan project, store items, return list
/// V02.T23: drift.list — query stored items by severity/type
use crate::drift::{scanner, storage};
use crate::AppContext;
use anyhow::Result;
use serde_json::Value;
use std::path::Path;
use tracing::debug;

/// drift.scan — run a fresh drift scan on a project.
///
/// Params: { "project_path": "/path/to/project" }
/// Returns: { "items": [...], "count": N }
pub async fn scan(params: Value, ctx: &AppContext) -> Result<Value> {
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();

    let path = Path::new(&project_path);
    debug!(project = %project_path, "drift.scan requested");

    let items = scanner::scan(path).await?;
    let count = items.len();

    let pool = ctx.storage.clone_pool();
    storage::clear_unresolved(&pool, &project_path).await?;
    if !items.is_empty() {
        storage::upsert_items(&pool, &items).await?;
    }

    // Emit push event when items found
    if count > 0 {
        ctx.broadcaster.broadcast(
            "session.driftDetected",
            serde_json::json!({
                "project_path": project_path,
                "count": count,
                "new_items": count,
            }),
        );
    }

    let items_json: Vec<Value> = items
        .iter()
        .map(|item| {
            serde_json::json!({
                "id": item.id,
                "feature": item.feature,
                "severity": item.severity.as_str(),
                "kind": item.kind,
                "message": item.message,
                "location": item.location,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "items": items_json,
        "count": count,
    }))
}

/// drift.list — query stored drift items for a project.
///
/// Params: { "project_path": "/path", "severity": "high" (optional) }
/// Returns: { "items": [...], "count": N }
pub async fn list(params: Value, ctx: &AppContext) -> Result<Value> {
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();

    let severity_filter = params
        .get("severity")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let pool = ctx.storage.clone_pool();
    let items = storage::list_items(&pool, &project_path, severity_filter.as_deref()).await?;

    let count = items.len();
    let items_json: Vec<Value> = items
        .iter()
        .map(|item| {
            serde_json::json!({
                "id": item.id,
                "feature": item.feature,
                "severity": item.severity.as_str(),
                "kind": item.kind,
                "message": item.message,
                "location": item.location,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "items": items_json,
        "count": count,
    }))
}
