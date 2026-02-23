//! Device Pairing data model types.

use serde::{Deserialize, Serialize};

/// A device that has completed the pairing flow and holds a long-lived token.
///
/// The `device_token` field is a 32-char hex string (UUID v4, dashes stripped).
/// It is stored as plain text in SQLite — callers compare it using
/// [`crate::pairing::storage::PairingStorage::get_by_token`].
///
/// **Never send this struct to a client over the wire.** Use [`PairedDevicePublic`]
/// for all outbound JSON — it omits the secret token.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    /// 32-char hex token — only visible server-side and in the initial pair response.
    pub device_token: String,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
    /// `0` = active, `1` = revoked (SQLite INTEGER).
    pub revoked: i64,
}

impl PairedDevice {
    /// Returns `true` if this device has been revoked.
    pub fn is_revoked(&self) -> bool {
        self.revoked != 0
    }
}

/// Public view of a paired device — safe to send to any connected client.
///
/// Identical to [`PairedDevice`] but with `device_token` stripped and
/// `revoked` converted to a `bool` for cleaner JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct PairedDevicePublic {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
    pub revoked: bool,
}

impl From<PairedDevice> for PairedDevicePublic {
    fn from(d: PairedDevice) -> Self {
        let revoked = d.is_revoked();
        Self {
            id: d.id,
            name: d.name,
            platform: d.platform,
            created_at: d.created_at,
            last_seen_at: d.last_seen_at,
            revoked,
        }
    }
}

/// A short-lived one-time pairing PIN record stored in `pair_pins`.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PinRecord {
    pub pin: String,
    pub expires_at: i64,
    /// `0` = unused, `1` = consumed (SQLite INTEGER).
    pub used: i64,
}

impl PinRecord {
    /// Returns `true` if this PIN has already been consumed.
    pub fn is_used(&self) -> bool {
        self.used != 0
    }
}

/// Parameters for the `device.pair` RPC — sent by the remote device.
#[derive(Debug, Deserialize)]
pub struct PairRequest {
    /// The 6-digit PIN shown on the desktop host (from `daemon.pairPin`).
    pub pin: String,
    /// Human-readable label for the device, e.g. "My iPhone".
    pub name: String,
    /// Platform identifier: "ios", "android", "macos", "windows", "linux", or "web".
    pub platform: String,
}

/// Successful response to `device.pair` — returned once to the pairing device.
///
/// The caller must store `device_token` securely (Keychain / EncryptedSharedPreferences).
/// It will not be retrievable again after this response.
#[derive(Debug, Serialize)]
pub struct PairResponse {
    /// ULID assigned to this device in `paired_devices.id`.
    pub device_id: String,
    /// 32-char hex token — the device's long-lived credential.
    pub device_token: String,
    /// Human-readable name of the host machine (for display in the device UI).
    pub host_name: String,
    /// Stable hardware fingerprint of the daemon instance.
    pub daemon_id: String,
    /// WebSocket relay URL the device should connect to for off-LAN access.
    pub relay_url: String,
}
