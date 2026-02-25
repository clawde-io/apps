//! SQLite persistence for the Device Pairing module.

use anyhow::Result;
use rand_core::{OsRng, RngCore};
use sqlx::SqlitePool;
use ulid::Ulid;
use uuid::Uuid;

use super::model::{PairedDevice, PairedDevicePublic};

/// PIN time-to-live in seconds (10 minutes).
const PIN_TTL_SECS: i64 = 600;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn unixepoch() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Generate a cryptographically random 6-digit PIN string ("100000"–"999999").
///
/// Uses [`OsRng`] (a thin wrapper around the OS CSPRNG) to fill a `u32`, then
/// reduces it into the 100 000–999 999 range with rejection sampling to avoid
/// modulo bias.
fn random_six_digit_pin() -> String {
    // Rejection-sampling loop: discard values that would introduce modulo bias.
    // On average fewer than 2 iterations are needed.
    let range: u32 = 900_000; // 999999 - 100000 + 1
    let threshold = u32::MAX - (u32::MAX % range);
    loop {
        let n = OsRng.next_u32();
        if n < threshold {
            return format!("{:06}", 100_000 + (n % range));
        }
    }
}

/// Generate a 32-char hex device token (UUID v4, dashes stripped).
fn random_device_token() -> String {
    Uuid::new_v4().simple().to_string()
}

// ─── PairingStorage ───────────────────────────────────────────────────────────

pub struct PairingStorage {
    pool: SqlitePool,
}

impl PairingStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ─── PINs ─────────────────────────────────────────────────────────────

    /// Generate a new cryptographically random 6-digit PIN, persist it with a
    /// 10-minute TTL, and return the PIN string.
    ///
    /// Expired and already-used PINs are pruned before the new one is inserted
    /// to keep the table lean.
    pub async fn generate_pin(&self) -> Result<String> {
        self.cleanup_expired_pins().await?;

        let pin = random_six_digit_pin();
        let expires_at = unixepoch() + PIN_TTL_SECS;

        sqlx::query(
            "INSERT INTO pair_pins (pin, expires_at, used) VALUES (?, ?, 0) \
             ON CONFLICT(pin) DO UPDATE SET expires_at = excluded.expires_at, used = 0",
        )
        .bind(&pin)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(pin)
    }

    /// Validate a PIN and atomically mark it consumed.
    ///
    /// Returns `true` if the PIN existed, was unused, and had not expired —
    /// and has now been marked `used = 1`.  Returns `false` in all other cases
    /// (expired, already used, or never issued).
    pub async fn validate_and_consume_pin(&self, pin: &str) -> Result<bool> {
        let now = unixepoch();
        let result = sqlx::query(
            "UPDATE pair_pins SET used = 1 \
             WHERE pin = ? AND used = 0 AND expires_at > ?",
        )
        .bind(pin)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // ─── Devices ──────────────────────────────────────────────────────────

    /// Issue a new device token and persist the device record.
    ///
    /// Returns the full [`PairedDevice`] including the secret token — the
    /// caller is responsible for sending it exactly once to the pairing device.
    pub async fn issue_device_token(&self, name: &str, platform: &str) -> Result<PairedDevice> {
        let id = Ulid::new().to_string();
        let device_token = random_device_token();
        let now = unixepoch();

        sqlx::query(
            "INSERT INTO paired_devices \
             (id, name, platform, device_token, created_at, revoked) \
             VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind(&id)
        .bind(name)
        .bind(platform)
        .bind(&device_token)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(sqlx::query_as::<_, PairedDevice>(
            "SELECT id, name, platform, device_token, created_at, last_seen_at, revoked \
             FROM paired_devices WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await?)
    }

    /// List all non-revoked devices (public view — no tokens).
    pub async fn list_devices(&self) -> Result<Vec<PairedDevicePublic>> {
        let rows = sqlx::query_as::<_, PairedDevice>(
            "SELECT id, name, platform, device_token, created_at, last_seen_at, revoked \
             FROM paired_devices \
             WHERE revoked = 0 \
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(PairedDevicePublic::from).collect())
    }

    /// Look up a device by its token.  Updates `last_seen_at` on a hit.
    ///
    /// Used by the auth middleware to validate incoming bearer tokens from
    /// paired devices.  Returns `None` if the token is unknown or revoked.
    ///
    /// The returned row reflects the `last_seen_at` timestamp written by this
    /// call (i.e. it is re-fetched after the UPDATE so the value is current).
    pub async fn get_by_token(&self, token: &str) -> Result<Option<PairedDevice>> {
        // First check whether the device exists and is not revoked.
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM paired_devices WHERE device_token = ? AND revoked = 0")
                .bind(token)
                .fetch_optional(&self.pool)
                .await?;

        let Some((id,)) = exists else {
            return Ok(None);
        };

        // Update last_seen_at atomically, then return the refreshed row.
        let now = unixepoch();
        sqlx::query("UPDATE paired_devices SET last_seen_at = ? WHERE id = ?")
            .bind(now)
            .bind(&id)
            .execute(&self.pool)
            .await?;

        let row = sqlx::query_as::<_, PairedDevice>(
            "SELECT id, name, platform, device_token, created_at, last_seen_at, revoked \
             FROM paired_devices WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Revoke a device by its ULID.
    ///
    /// Returns `true` if the row was found and updated, `false` if the device
    /// id does not exist or was already revoked.
    pub async fn revoke_device(&self, id: &str) -> Result<bool> {
        let result =
            sqlx::query("UPDATE paired_devices SET revoked = 1 WHERE id = ? AND revoked = 0")
                .bind(id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Rename a paired device.
    ///
    /// Returns `true` if the row was found and the name was updated.
    pub async fn rename_device(&self, id: &str, name: &str) -> Result<bool> {
        let result = sqlx::query("UPDATE paired_devices SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Check whether a PIN row exists in the table (regardless of used/expired state).
    ///
    /// Used by the handler to distinguish "PIN never issued" (→ INVALID) from
    /// "PIN consumed or expired" (→ EXPIRED) after `validate_and_consume_pin`
    /// returns `false`.
    pub async fn pin_row_exists(&self, pin: &str) -> Result<bool> {
        let row: Option<(String,)> = sqlx::query_as("SELECT pin FROM pair_pins WHERE pin = ?")
            .bind(pin)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    // ─── Maintenance ──────────────────────────────────────────────────────

    /// Delete all expired or already-consumed PINs and return the count removed.
    ///
    /// Called automatically by [`generate_pin`] before each new insertion.
    pub async fn cleanup_expired_pins(&self) -> Result<u64> {
        let now = unixepoch();
        let result = sqlx::query("DELETE FROM pair_pins WHERE used = 1 OR expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePool::connect_with(opts).await.unwrap();

        // Create the tables needed for the pairing module.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pair_pins (
                pin        TEXT PRIMARY KEY,
                expires_at INTEGER NOT NULL,
                used       INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS paired_devices (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                platform     TEXT NOT NULL,
                device_token TEXT NOT NULL UNIQUE,
                created_at   INTEGER NOT NULL,
                last_seen_at INTEGER,
                revoked      INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_paired_devices_token ON paired_devices(device_token)",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_generate_pin() {
        let storage = PairingStorage::new(test_pool().await);
        let pin = storage.generate_pin().await.unwrap();

        // Must be exactly 6 digits.
        assert_eq!(pin.len(), 6, "PIN must be 6 characters");
        assert!(
            pin.chars().all(|c| c.is_ascii_digit()),
            "PIN must be numeric"
        );

        let n: u32 = pin.parse().unwrap();
        assert!(
            (100_000..=999_999).contains(&n),
            "PIN must be in range 100000–999999"
        );
    }

    #[tokio::test]
    async fn test_validate_and_consume_pin() {
        let storage = PairingStorage::new(test_pool().await);
        let pin = storage.generate_pin().await.unwrap();

        // First validation must succeed.
        let ok = storage.validate_and_consume_pin(&pin).await.unwrap();
        assert!(ok, "first validation of a fresh PIN should succeed");

        // Second validation of the same PIN must fail (already consumed).
        let ok2 = storage.validate_and_consume_pin(&pin).await.unwrap();
        assert!(!ok2, "second validation of the same PIN should fail");

        // Validation of a PIN that was never issued must also fail.
        let bad = storage.validate_and_consume_pin("000000").await.unwrap();
        assert!(!bad, "unknown PIN must not validate");
    }

    #[tokio::test]
    async fn test_issue_device_token() {
        let storage = PairingStorage::new(test_pool().await);
        let device = storage
            .issue_device_token("Test Phone", "ios")
            .await
            .unwrap();

        assert!(!device.id.is_empty(), "device id must not be empty");
        assert_eq!(device.name, "Test Phone");
        assert_eq!(device.platform, "ios");
        assert_eq!(
            device.device_token.len(),
            32,
            "device_token must be 32 hex chars"
        );
        assert!(
            device.device_token.chars().all(|c| c.is_ascii_hexdigit()),
            "device_token must be lowercase hex"
        );
        assert!(
            !device.is_revoked(),
            "newly issued device must not be revoked"
        );
    }

    #[tokio::test]
    async fn test_list_devices() {
        let storage = PairingStorage::new(test_pool().await);

        // Issue two devices.
        let d1 = storage.issue_device_token("Phone A", "ios").await.unwrap();
        let d2 = storage
            .issue_device_token("Phone B", "android")
            .await
            .unwrap();

        let list = storage.list_devices().await.unwrap();
        assert_eq!(list.len(), 2);

        // Revoke one; it must disappear from the list.
        storage.revoke_device(&d1.id).await.unwrap();
        let list2 = storage.list_devices().await.unwrap();
        assert_eq!(list2.len(), 1);
        assert_eq!(list2[0].id, d2.id);
    }

    #[tokio::test]
    async fn test_revoke_device() {
        let storage = PairingStorage::new(test_pool().await);
        let device = storage.issue_device_token("My Mac", "macos").await.unwrap();

        // Revoking an active device must return true.
        let revoked = storage.revoke_device(&device.id).await.unwrap();
        assert!(revoked, "revoking an active device must return true");

        // Revoking again must return false (idempotent but no-op).
        let revoked2 = storage.revoke_device(&device.id).await.unwrap();
        assert!(
            !revoked2,
            "revoking an already-revoked device must return false"
        );

        // The token must no longer be found via get_by_token.
        let found = storage.get_by_token(&device.device_token).await.unwrap();
        assert!(found.is_none(), "revoked device must not be found by token");
    }

    #[tokio::test]
    async fn test_get_by_token_updates_last_seen() {
        let storage = PairingStorage::new(test_pool().await);
        let device = storage
            .issue_device_token("Tablet", "android")
            .await
            .unwrap();

        // last_seen_at should be None initially.
        assert!(device.last_seen_at.is_none());

        // After a lookup, last_seen_at must be populated.
        let found = storage
            .get_by_token(&device.device_token)
            .await
            .unwrap()
            .unwrap();
        assert!(
            found.last_seen_at.is_some(),
            "get_by_token must update last_seen_at"
        );
    }

    #[tokio::test]
    async fn test_rename_device() {
        let storage = PairingStorage::new(test_pool().await);
        let device = storage
            .issue_device_token("Old Name", "macos")
            .await
            .unwrap();

        let ok = storage.rename_device(&device.id, "New Name").await.unwrap();
        assert!(ok, "rename_device must return true for an existing device");

        // Verify via get_by_token.
        let found = storage
            .get_by_token(&device.device_token)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.name, "New Name");
    }

    #[tokio::test]
    async fn test_cleanup_expired_pins() {
        let storage = PairingStorage::new(test_pool().await);

        // Insert a pin that is already expired (expires_at in the past).
        let past = unixepoch() - 1;
        sqlx::query("INSERT INTO pair_pins (pin, expires_at, used) VALUES ('999999', ?, 0)")
            .bind(past)
            .execute(&storage.pool)
            .await
            .unwrap();

        // Insert a fresh, unused pin.
        let future = unixepoch() + PIN_TTL_SECS;
        sqlx::query("INSERT INTO pair_pins (pin, expires_at, used) VALUES ('111111', ?, 0)")
            .bind(future)
            .execute(&storage.pool)
            .await
            .unwrap();

        let removed = storage.cleanup_expired_pins().await.unwrap();
        assert_eq!(removed, 1, "only the expired pin must be removed");

        // The fresh pin must still be present.
        let row: Option<(String,)> =
            sqlx::query_as("SELECT pin FROM pair_pins WHERE pin = '111111'")
                .fetch_optional(&storage.pool)
                .await
                .unwrap();
        assert!(row.is_some(), "unexpired pin must survive cleanup");
    }
}
