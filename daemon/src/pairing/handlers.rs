//! RPC handlers for the Device Pairing module.
//!
//! RPC method → handler mapping (registered externally in `ipc/mod.rs`):
//!
//! | Method            | Handler             | Who calls it                   |
//! |-------------------|---------------------|--------------------------------|
//! | `daemon.pairPin`  | `pairing_generate_pin` | Desktop (shows QR / PIN)    |
//! | `device.pair`     | `device_pair`       | Mobile / web (enter PIN)       |
//! | `device.list`     | `device_list`       | Desktop (manage paired devices)|
//! | `device.revoke`   | `device_revoke`     | Desktop                        |
//! | `device.rename`   | `device_rename`     | Desktop                        |

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

use super::model::PairRequest;
use super::storage::PairingStorage;

// ─── Error codes ─────────────────────────────────────────────────────────────

/// `device.pair` — the PIN was never issued or does not match any record.
pub const PAIR_PIN_INVALID: i64 = -32021;
/// `device.pair` — the PIN exists but has already been used or has expired.
pub const PAIR_PIN_EXPIRED: i64 = -32022;
/// `device.revoke` / `device.rename` — no device with the given id exists.
pub const DEVICE_NOT_FOUND: i64 = -32020;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn pairing_storage(ctx: &AppContext) -> PairingStorage {
    PairingStorage::new(ctx.storage.pool())
}

/// Resolve the human-readable hostname to display in the pairing UI.
///
/// Priority: `HOSTNAME` env var → `COMPUTERNAME` env var (Windows) → `"clawd"`.
fn host_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "clawd".to_string())
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// `daemon.pairPin` — generate a new 6-digit PIN and return pairing metadata.
///
/// Called by the desktop app to display a PIN / QR code to the user.  The
/// device app must call `device.pair` with the PIN within 10 minutes.
pub async fn pairing_generate_pin(_params: Value, ctx: &AppContext) -> Result<Value> {
    let pin = pairing_storage(ctx).generate_pin().await?;
    let relay_url = ctx.config.relay_url.clone();
    let host = host_name();

    Ok(json!({
        "pin": pin,
        "expires_in_seconds": 600,
        "daemon_id": ctx.daemon_id,
        "relay_url": relay_url,
        "host_name": host
    }))
}

/// `device.pair` — consume a PIN and issue a long-lived device token.
///
/// Called by the device (mobile app, web client) after the user enters or
/// scans the PIN shown on the desktop.  On success the response contains the
/// `device_token` that must be stored securely on the device — it will not be
/// retrievable again.
pub async fn device_pair(params: Value, ctx: &AppContext) -> Result<Value> {
    let req: PairRequest = serde_json::from_value(params)?;

    let storage = pairing_storage(ctx);

    // Validate that the PIN exists and hasn't been used/expired.
    // `validate_and_consume_pin` atomically marks it used on success.
    let consumed = storage.validate_and_consume_pin(&req.pin).await?;

    if !consumed {
        // Distinguish "PIN never existed" from "PIN expired / already used".
        // If the row still exists in the table it was consumed or expired;
        // if the row is absent it was never issued (or was cleaned up by the
        // last generate_pin call).
        let msg = if storage.pin_row_exists(&req.pin).await? {
            "PAIR_PIN_EXPIRED: pin expired or already used"
        } else {
            "PAIR_PIN_INVALID: pin rejected"
        };
        anyhow::bail!("{}", msg);
    }

    // Issue a new device token and persist the device record.
    let device = storage.issue_device_token(&req.name, &req.platform).await?;

    // Broadcast `device.paired` to all connected clients (desktop UI refresh).
    // The event does NOT include the device token — only public metadata.
    ctx.broadcaster.broadcast(
        "device.paired",
        json!({
            "device_id": device.id,
            "name": device.name,
            "platform": device.platform
        }),
    );

    let relay_url = ctx.config.relay_url.clone();
    let host = host_name();

    Ok(serde_json::to_value(super::model::PairResponse {
        device_id: device.id,
        device_token: device.device_token,
        host_name: host,
        daemon_id: ctx.daemon_id.clone(),
        relay_url,
    })?)
}

/// `device.list` — return all non-revoked paired devices (public view).
pub async fn device_list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let devices = pairing_storage(ctx).list_devices().await?;
    Ok(json!({ "devices": devices }))
}

/// `device.revoke` — revoke a paired device so its token no longer grants access.
///
/// Params: `{ "id": "<device-ulid>" }`
pub async fn device_revoke(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("DEVICE_NOT_FOUND: missing `id` parameter"))?;

    let revoked = pairing_storage(ctx).revoke_device(id).await?;

    if !revoked {
        anyhow::bail!("DEVICE_NOT_FOUND: no active device with id {}", id);
    }

    // Notify connected clients (desktop list refresh).
    ctx.broadcaster
        .broadcast("device.revoked", json!({ "device_id": id }));

    Ok(json!({ "revoked": true }))
}

/// `device.rename` — update the human-readable label of a paired device.
///
/// Params: `{ "id": "<device-ulid>", "name": "<new label>" }`
pub async fn device_rename(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("DEVICE_NOT_FOUND: missing `id` parameter"))?;
    let name = params["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing `name` parameter"))?;

    let ok = pairing_storage(ctx).rename_device(id, name).await?;

    if !ok {
        anyhow::bail!("DEVICE_NOT_FOUND: no device with id {}", id);
    }

    Ok(json!({ "ok": true }))
}
