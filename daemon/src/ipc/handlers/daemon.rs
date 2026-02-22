use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

pub async fn ping(_params: Value, _ctx: &AppContext) -> Result<Value> {
    Ok(json!({ "pong": true }))
}

pub async fn status(_params: Value, ctx: &AppContext) -> Result<Value> {
    let uptime = ctx.started_at.elapsed().as_secs();
    let active_sessions = ctx.session_manager.active_count().await;
    let watched_repos = ctx.repo_registry.watched_count().await;
    let pending_update = ctx.updater.pending_update().await.map(|u| u.version);
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "daemonId": ctx.daemon_id,
        "uptime": uptime,
        "activeSessions": active_sessions,
        "watchedRepos": watched_repos,
        "port": ctx.config.port,
        "pendingUpdate": pending_update
    }))
}

pub async fn check_update(_params: Value, ctx: &AppContext) -> Result<Value> {
    let (current, latest, available) = ctx.updater.check().await?;
    Ok(json!({
        "current": current,
        "latest": latest,
        "available": available
    }))
}

pub async fn apply_update(_params: Value, ctx: &AppContext) -> Result<Value> {
    // Refuse to apply an update while sessions are active to avoid interrupting
    // in-flight AI turns.  The Flutter UI should check activeSessions first.
    let active = ctx.session_manager.active_count().await;
    if active > 0 {
        return Err(anyhow::anyhow!(
            "SESSION_BUSY: {} active session(s) — wait for them to finish before updating",
            active
        ));
    }
    let applied = ctx.updater.apply_if_ready().await?;
    Ok(json!({ "applied": applied }))
}
