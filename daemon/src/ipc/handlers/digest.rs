//! Sprint EE DD.2 — `digest.today` RPC handler.

use crate::AppContext;
use anyhow::Result;
use serde_json::Value;

/// `digest.today` — Return the daily digest for today.
pub async fn today(_params: Value, ctx: &AppContext) -> Result<Value> {
    crate::scheduler::digest::today_response(ctx.storage.pool()).await
}
