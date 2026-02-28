// rest/routes/metrics.rs — GET /api/v1/metrics (Sprint QQ RA.5).

use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppContext;

pub async fn get_metrics(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let since = now - 86400;

    match ctx.metrics_store.summary(since, now).await {
        Ok(s) => Json(json!({
            "total_tokens_in": s.total_tokens_in,
            "total_tokens_out": s.total_tokens_out,
            "total_tool_calls": s.total_tool_calls,
            "total_cost_usd": s.total_cost_usd,
            "session_count": s.session_count,
            "period_start": s.period_start,
            "period_end": s.period_end,
        })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
