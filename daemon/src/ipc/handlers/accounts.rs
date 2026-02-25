use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// account.list — return all configured accounts ordered by priority.
pub async fn list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let accounts = ctx.storage.list_accounts().await?;
    let items: Vec<Value> = accounts
        .into_iter()
        .map(|a| {
            json!({
                "id": a.id,
                "name": a.name,
                "provider": a.provider,
                "credentialsPath": a.credentials_path,
                "priority": a.priority,
                "limitedUntil": a.limited_until,
            })
        })
        .collect();
    Ok(json!({ "accounts": items }))
}

/// account.create — add a new account to the pool.
///
/// Params: { name, provider, credentialsPath, priority? }
pub async fn create(params: Value, ctx: &AppContext) -> Result<Value> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("INVALID_PARAMS: missing name"))?;
    let provider = params["provider"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("INVALID_PARAMS: missing provider"))?;
    let credentials_path = params["credentialsPath"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("INVALID_PARAMS: missing credentialsPath"))?;
    let priority = params["priority"].as_i64().unwrap_or(100);

    let account = ctx
        .storage
        .create_account(name, provider, credentials_path, priority)
        .await?;

    ctx.storage
        .log_account_event(&account.id, "created", None)
        .await?;

    Ok(json!({
        "account": {
            "id": account.id,
            "name": account.name,
            "provider": account.provider,
            "credentialsPath": account.credentials_path,
            "priority": account.priority,
            "limitedUntil": account.limited_until,
        }
    }))
}

/// account.delete — remove an account by id.
///
/// Params: { id }
pub async fn delete(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("INVALID_PARAMS: missing id"))?;

    // Verify account exists before deleting.
    ctx.storage
        .get_account(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("ACCOUNT_NOT_FOUND: {}", id))?;

    ctx.storage.delete_account(id).await?;
    ctx.storage.log_account_event(id, "deleted", None).await?;

    Ok(json!({ "deleted": true }))
}

/// account.setPriority — update scheduling priority for an account.
///
/// Lower numbers = higher priority (picked first by the scheduler).
/// Params: { id, priority }
pub async fn set_priority(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("INVALID_PARAMS: missing id"))?;
    let priority = params["priority"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("INVALID_PARAMS: missing priority"))?;

    ctx.storage
        .get_account(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("ACCOUNT_NOT_FOUND: {}", id))?;

    let old_account = ctx
        .storage
        .get_account(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("ACCOUNT_NOT_FOUND: {}", id))?;

    ctx.storage.update_account_priority(id, priority).await?;

    let meta = serde_json::to_string(&json!({
        "oldPriority": old_account.priority,
        "newPriority": priority,
    }))?;
    ctx.storage
        .log_account_event(id, "priority_changed", Some(&meta))
        .await?;

    Ok(json!({ "updated": true, "id": id, "priority": priority }))
}

/// account.history — recent events for all accounts or a specific one.
///
/// Params: { accountId?, limit? }
pub async fn history(params: Value, ctx: &AppContext) -> Result<Value> {
    let account_id = params["accountId"].as_str();
    let limit = params["limit"].as_i64().unwrap_or(50).clamp(1, 500);

    let events = ctx
        .storage
        .list_account_events(account_id, limit)
        .await?;

    let items: Vec<Value> = events
        .into_iter()
        .map(|e| {
            json!({
                "id": e.id,
                "accountId": e.account_id,
                "eventType": e.event_type,
                "metadata": e.metadata,
                "createdAt": e.created_at,
            })
        })
        .collect();

    Ok(json!({ "events": items }))
}
