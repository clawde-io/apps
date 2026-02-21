use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

pub async fn ping(_params: Value, _ctx: &AppContext) -> Result<Value> {
    Ok(json!({ "pong": true }))
}

pub async fn status(_params: Value, ctx: &AppContext) -> Result<Value> {
    let uptime = ctx.started_at.elapsed().as_secs();
    let active_sessions = ctx.session_manager.active_count().await;
    let watched_repos = ctx.repo_registry.watched_count();
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": uptime,
        "activeSessions": active_sessions,
        "watchedRepos": watched_repos,
        "port": ctx.config.port
    }))
}
