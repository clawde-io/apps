// ipc/handlers/push.rs — Push notification device token registration (Sprint RR PN.3).
//
// RPC: push.register
//   params: { device_id, token, platform: "apns" | "fcm" }
//   result: { registered: true }
//
// Stores the device token in the `push_tokens` table so the relay can
// forward session events as push notifications (iOS APNs + Android FCM).

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

pub async fn register(params: Value, ctx: &AppContext) -> Result<Value> {
    let device_id = params["device_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing device_id"))?
        .to_string();
    let token = params["token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing token"))?
        .to_string();
    let platform = params["platform"].as_str().unwrap_or("fcm").to_string();

    // Persist to storage so the relay can look it up
    ctx.storage
        .upsert_push_token(&device_id, &token, &platform)
        .await?;

    info!(device_id, platform, "push token registered");

    Ok(json!({ "registered": true }))
}

pub async fn unregister(params: Value, ctx: &AppContext) -> Result<Value> {
    let device_id = params["device_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing device_id"))?;

    ctx.storage.delete_push_token(device_id).await?;
    info!(device_id, "push token unregistered");

    Ok(json!({ "unregistered": true }))
}
