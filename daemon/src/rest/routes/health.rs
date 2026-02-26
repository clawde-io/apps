use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::AppContext;

pub async fn health(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    let uptime = ctx.started_at.elapsed().as_secs();
    Json(json!({
        "status": "ok",
        "daemon_id": ctx.daemon_id,
        "uptime_secs": uptime,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
