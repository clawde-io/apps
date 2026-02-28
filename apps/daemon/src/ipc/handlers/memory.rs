// ipc/handlers/memory.rs — memory.list, memory.add, memory.remove, memory.update RPCs.
//
// Sprint OO ME.8

use anyhow::Result;
use serde_json::{json, Value};

use crate::ipc::AppContext;
use crate::memory::store::AddMemoryRequest;
use crate::memory::MemoryStore;

/// `memory.list` — List memory entries for a scope.
///
/// Params: `{ scope?: string, project_scope?: string }`
/// - scope: explicit scope string ("global" or "proj:...")
/// - project_scope: repo_path — will be converted to scope hash
pub async fn list(params: Value, ctx: &AppContext) -> Result<Value> {
    let scope = if let Some(repo_path) = params.get("repo_path").and_then(|v| v.as_str()) {
        MemoryStore::project_scope(repo_path)
    } else {
        params
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("global")
            .to_string()
    };

    let include_global = params
        .get("include_global")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let entries = if include_global && scope != "global" {
        ctx.memory_store.list_all(&scope).await?
    } else {
        ctx.memory_store.list(&scope).await?
    };

    Ok(json!({
        "entries": entries,
        "count": entries.len(),
    }))
}

/// `memory.add` — Add or update a memory entry.
///
/// Params: `{ scope?, repo_path?, key, value, weight?, source? }`
pub async fn add(params: Value, ctx: &AppContext) -> Result<Value> {
    let scope = if let Some(repo_path) = params.get("repo_path").and_then(|v| v.as_str()) {
        MemoryStore::project_scope(repo_path)
    } else {
        params
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("global")
            .to_string()
    };

    let key = params
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("key is required"))?
        .to_string();

    let value = params
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("value is required"))?
        .to_string();

    let weight = params.get("weight").and_then(|v| v.as_i64());
    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let entry = ctx
        .memory_store
        .upsert(AddMemoryRequest {
            scope,
            key,
            value,
            weight,
            source,
        })
        .await?;

    Ok(json!({ "entry": entry }))
}

/// `memory.remove` — Delete a memory entry by ID.
///
/// Params: `{ id: string }`
pub async fn remove(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("id is required"))?;

    let removed = ctx.memory_store.remove(id).await?;
    Ok(json!({ "removed": removed }))
}

/// `memory.update` — Update weight or value of an existing entry by ID.
///
/// Params: `{ scope, key, value?, weight? }`
/// Uses upsert — updates existing entry if scope+key matches.
pub async fn update(params: Value, ctx: &AppContext) -> Result<Value> {
    // Delegate to add — upsert on conflict
    add(params, ctx).await
}
