// SPDX-License-Identifier: MIT
//! RPC handlers for license verification and tier gating.
//!
//! Exposes:
//!   `license.get`     — return current cached license info
//!   `license.check`   — re-verify with the ClawDE backend and update cache
//!   `license.tier`    — quick read: current tier string only

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `license.get` — return current cached license tier and feature flags.
///
/// Params: (none)
/// Returns:
/// ```json
/// {
///   "tier": "personal_remote",
///   "features": { "relay": true, "autoSwitch": true },
///   "cached": true
/// }
/// ```
pub async fn get(_params: Value, ctx: &AppContext) -> Result<Value> {
    let info = ctx.license.read().await;
    Ok(json!({
        "tier": info.tier,
        "features": {
            "relay": info.features.relay,
            "autoSwitch": info.features.auto_switch,
        },
        "cached": true,
    }))
}

/// `license.check` — re-verify with the ClawDE backend and refresh the in-memory license.
///
/// Makes a live network call to POST /daemon/verify.
/// Falls back to cache (24-hour grace period) if the call fails.
///
/// Params: (none)
/// Returns: same shape as `license.get`, plus `"refreshed": bool`
pub async fn check(_params: Value, ctx: &AppContext) -> Result<Value> {
    let daemon_id = ctx.daemon_id.clone();

    let fresh = crate::license::verify_and_cache(&ctx.storage, &ctx.config, &daemon_id).await;

    let refreshed = !fresh.tier.is_empty();

    // Update the in-memory license.
    {
        let mut guard = ctx.license.write().await;
        *guard = fresh.clone();
    }

    Ok(json!({
        "tier": fresh.tier,
        "features": {
            "relay": fresh.features.relay,
            "autoSwitch": fresh.features.auto_switch,
        },
        "cached": false,
        "refreshed": refreshed,
    }))
}

/// `license.tier` — quick read of the current license tier string.
///
/// Params: (none)
/// Returns: `{ "tier": "free" }`
pub async fn tier(_params: Value, ctx: &AppContext) -> Result<Value> {
    let info = ctx.license.read().await;
    Ok(json!({ "tier": info.tier }))
}
