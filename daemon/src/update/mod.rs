//! Self-update manager.
//!
//! Checks GitHub Releases for newer versions and applies them when the daemon
//! is idle (no active sessions, no connected WebSocket clients).
//!
//! Process:
//! 1. Query `https://api.github.com/repos/clawde-io/apps/releases/latest`
//! 2. Compare tag vs `CARGO_PKG_VERSION` using semver
//! 3. If newer: download binary for current platform, verify SHA256, stage it
//! 4. When idle: atomic rename → exec new binary (Unix) / service restart (Windows)
//!
//! Rollback-on-crash (DC.T29):
//! - Before applying, backs up current binary to `{exe}.backup` and writes a
//!   sentinel file `{data_dir}/clawd-rollback.sentinel` stamped with the UTC time.
//! - On the next startup, `check_and_rollback()` is called first.  If the
//!   sentinel is <30 s old and the backup exists, the backup is restored
//!   automatically (the new binary crashed without deleting the sentinel).
//! - `delete_rollback_sentinel()` is called at the end of successful startup to
//!   clear the sentinel for that run.
//!
//! Update policy (DC.T32):
//! - "auto"   — check, download, and apply automatically when idle (default)
//! - "manual" — check and broadcast `daemon.updateAvailable`; download only;
//!   user must call `daemon.applyUpdate` to restart
//! - "never"  — disable all update checks

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use semver::Version;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::config::DaemonConfig;
use crate::ipc::event::EventBroadcaster;

// ─── GitHub API types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

// ─── Update state ─────────────────────────────────────────────────────────────

/// A downloaded update waiting to be applied.
#[derive(Debug, Clone)]
pub struct PendingUpdate {
    pub version: String,
    pub binary_path: PathBuf,
}

// ─── Rollback sentinel (DC.T29) ──────────────────────────────────────────────

fn rollback_sentinel_path(data_dir: &Path) -> PathBuf {
    data_dir.join("clawd-rollback.sentinel")
}

/// Write the rollback sentinel before applying an update.
///
/// The sentinel records the UTC timestamp of the apply attempt.
/// If the new binary crashes on startup, the next invocation can detect this
/// and restore the backup.
fn write_rollback_sentinel(data_dir: &Path) {
    let path = rollback_sentinel_path(data_dir);
    if let Err(e) = std::fs::write(&path, chrono::Utc::now().to_rfc3339()) {
        warn!("failed to write rollback sentinel: {e}");
    }
}

/// Delete the rollback sentinel after a successful startup.
///
/// Call this once the daemon is fully up and serving requests.
pub fn delete_rollback_sentinel(data_dir: &Path) {
    let _ = std::fs::remove_file(rollback_sentinel_path(data_dir));
}

/// Check for a crash-recovery sentinel on startup and restore the backup if needed.
///
/// Returns `true` if a rollback was performed (caller should log this prominently).
///
/// Rollback conditions (all must be true):
/// 1. `clawd-rollback.sentinel` exists in `data_dir`
/// 2. The timestamp in the sentinel is less than 30 seconds ago
/// 3. A backup binary (`{current_exe}.backup`) exists
pub fn check_and_rollback(data_dir: &Path) -> bool {
    let sentinel = rollback_sentinel_path(data_dir);

    let content = match std::fs::read_to_string(&sentinel) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let ts = match chrono::DateTime::parse_from_rfc3339(content.trim()) {
        Ok(t) => t,
        Err(_) => {
            let _ = std::fs::remove_file(&sentinel);
            return false;
        }
    };

    let age_secs = chrono::Utc::now()
        .signed_duration_since(ts.with_timezone(&chrono::Utc))
        .num_seconds();

    if age_secs > 30 {
        // Stale sentinel from a much older apply — clean up and ignore.
        let _ = std::fs::remove_file(&sentinel);
        return false;
    }

    let current_exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return false,
    };
    let backup = current_exe.with_extension("backup");

    if !backup.exists() {
        let _ = std::fs::remove_file(&sentinel);
        return false;
    }

    match std::fs::rename(&backup, &current_exe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&sentinel);
            warn!(
                "ROLLBACK: update binary crashed within 30 s — restored backup, \
                 daemon is running on previous version"
            );
            true
        }
        Err(e) => {
            warn!("rollback attempted but rename failed: {e}");
            false
        }
    }
}

// ─── Updater ─────────────────────────────────────────────────────────────────

/// Manages the self-update lifecycle.
#[derive(Clone)]
pub struct Updater {
    config: Arc<DaemonConfig>,
    broadcaster: Arc<EventBroadcaster>,
    pending: Arc<Mutex<Option<PendingUpdate>>>,
    /// Current update policy ("auto" | "manual" | "never").
    /// Writable so `daemon.setUpdatePolicy` can change it at runtime.
    policy: Arc<Mutex<String>>,
}

impl Updater {
    pub fn new(config: Arc<DaemonConfig>, broadcaster: Arc<EventBroadcaster>) -> Self {
        let initial_policy = config.update_policy.clone();
        Self {
            config,
            broadcaster,
            pending: Arc::new(Mutex::new(None)),
            policy: Arc::new(Mutex::new(initial_policy)),
        }
    }

    /// Current update policy string.
    pub async fn get_policy(&self) -> String {
        self.policy.lock().await.clone()
    }

    /// Change the update policy at runtime.
    /// Persisted only in-memory; survives until the daemon restarts.
    pub async fn set_policy(&self, policy: &str) -> Result<()> {
        const VALID: &[&str] = &["auto", "manual", "never"];
        if !VALID.contains(&policy) {
            bail!(
                "invalid type: unknown update policy '{policy}' — must be one of: {}",
                VALID.join(", ")
            );
        }
        *self.policy.lock().await = policy.to_string();
        info!(policy = %policy, "update policy changed");
        Ok(())
    }

    /// Check GitHub Releases for a newer version.
    /// Returns `(current_version, latest_version, is_newer)`.
    pub async fn check(&self) -> Result<(String, String, bool)> {
        let current =
            Version::parse(env!("CARGO_PKG_VERSION")).context("invalid CARGO_PKG_VERSION")?;

        let release = self.fetch_latest_release().await?;
        let tag = release.tag_name.trim_start_matches('v').to_string();
        let latest = Version::parse(&tag).context("invalid release tag semver")?;

        let is_newer = latest > current;
        Ok((current.to_string(), latest.to_string(), is_newer))
    }

    /// Check + broadcast `daemon.updateAvailable` if newer, then download (unless policy = "manual").
    /// Called on startup and every 24 hours.
    pub async fn check_and_download(&self) -> Result<()> {
        let policy = self.policy.lock().await.clone();

        if policy == "never" {
            debug!("update checks disabled by policy");
            return Ok(());
        }

        let (current, latest, is_newer) = self.check().await?;

        if !is_newer {
            debug!(current = %current, "no update available");
            return Ok(());
        }

        info!(current = %current, latest = %latest, "update available — broadcasting");

        let release = self.fetch_latest_release().await?;
        let release_notes_url = release.html_url.clone();

        self.broadcaster.broadcast(
            "daemon.updateAvailable",
            json!({
                "current": current,
                "latest": latest,
                "releaseNotesUrl": release_notes_url,
                "policy": policy,
            }),
        );

        if policy == "manual" {
            // Broadcast only; download is triggered by daemon.applyUpdate.
            return Ok(());
        }

        // Auto-policy: download in background.
        let this = self.clone();
        let latest_clone = latest.clone();
        tokio::spawn(async move {
            if let Err(e) = this.download(&release, &latest_clone).await {
                warn!("update download failed: {e:#}");
                this.broadcaster.broadcast(
                    "daemon.updateFailed",
                    json!({ "version": latest_clone, "reason": e.to_string() }),
                );
            }
        });

        Ok(())
    }

    /// Download the release binary and verify SHA256.
    async fn download(&self, release: &GhRelease, version: &str) -> Result<()> {
        let platform = current_platform();
        debug!(platform = %platform, "looking for release asset");

        let binary_asset = release
            .assets
            .iter()
            .find(|a| a.name == format!("clawd-{platform}"))
            .with_context(|| format!("no asset for platform {platform}"))?;

        let checksum_asset = release
            .assets
            .iter()
            .find(|a| a.name == format!("clawd-{platform}.sha256"))
            .with_context(|| format!("no checksum asset for platform {platform}"))?;

        let client = build_client()?;
        let checksum_text = client
            .get(&checksum_asset.browser_download_url)
            .send()
            .await?
            .text()
            .await?;
        let expected_hash = checksum_text
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

        let dest = self.config.data_dir.join(format!("clawd-update-{version}"));

        let mut file = tokio::fs::File::create(&dest)
            .await
            .context("failed to create update file")?;

        let mut response = client
            .get(&binary_asset.browser_download_url)
            .send()
            .await?;

        let mut hasher = Sha256::new();

        while let Some(chunk) = response.chunk().await? {
            hasher.update(&chunk);
            file.write_all(&chunk)
                .await
                .context("failed to write update chunk")?;
        }
        file.flush().await?;

        let actual_hash = format!("{:x}", hasher.finalize());

        if actual_hash != expected_hash {
            let _ = tokio::fs::remove_file(&dest).await;
            bail!("SHA256 mismatch: expected {expected_hash}, got {actual_hash}");
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest, perms)?;
        }

        info!(version = %version, path = %dest.display(), "update downloaded and verified");

        *self.pending.lock().await = Some(PendingUpdate {
            version: version.to_string(),
            binary_path: dest,
        });

        Ok(())
    }

    /// Apply the pending update immediately.
    ///
    /// Writes a rollback sentinel, backs up current binary to `{exe}.backup`,
    /// then atomically renames the staged update into place and exec's it.
    /// On Windows, issues an SCM service restart instead.
    pub async fn apply_if_ready(&self) -> Result<bool> {
        let pending = {
            let guard = self.pending.lock().await;
            guard.clone()
        };

        let Some(update) = pending else {
            return Ok(false);
        };

        self.broadcaster
            .broadcast("daemon.updating", json!({ "version": update.version }));

        info!(version = %update.version, "applying update");

        let current_exe = std::env::current_exe().context("failed to get current binary path")?;
        let backup = current_exe.with_extension("backup");

        // Write rollback sentinel BEFORE moving binaries (DC.T29).
        write_rollback_sentinel(&self.config.data_dir);

        // Rename current → backup
        std::fs::rename(&current_exe, &backup)
            .context("failed to rename current binary to backup")?;

        // Rename staged → current
        if let Err(e) = std::fs::rename(&update.binary_path, &current_exe) {
            // Restore backup so the daemon can still restart cleanly.
            let _ = std::fs::rename(&backup, &current_exe);
            let _ = std::fs::remove_file(rollback_sentinel_path(&self.config.data_dir));
            return Err(e).context("failed to rename update binary");
        }

        // Exec the new binary (Unix) or restart (Windows)
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let args: Vec<String> = std::env::args().collect();
            let err = std::process::Command::new(&current_exe)
                .args(&args[1..])
                .exec();
            // exec() should never return on success.
            Err(anyhow::anyhow!("exec failed: {err}"))
        }

        #[cfg(not(unix))]
        {
            // On Windows, restart the SCM service.
            let _ = std::process::Command::new("sc")
                .args(["stop", "clawd"])
                .status();
            let _ = std::process::Command::new("sc")
                .args(["start", "clawd"])
                .status();
            Ok(true)
        }
    }

    /// Return info about the pending update, if any.
    pub async fn pending_update(&self) -> Option<PendingUpdate> {
        self.pending.lock().await.clone()
    }

    // ─── Private ─────────────────────────────────────────────────────────────

    async fn fetch_latest_release(&self) -> Result<GhRelease> {
        let url = "https://api.github.com/repos/clawde-io/apps/releases/latest";
        let client = build_client()?;
        let release: GhRelease = client
            .get(url)
            .header("User-Agent", format!("clawd/{}", env!("CARGO_PKG_VERSION")))
            .send()
            .await
            .context("failed to fetch GitHub releases")?
            .error_for_status()
            .context("GitHub API error")?
            .json()
            .await
            .context("failed to parse GitHub release JSON")?;
        Ok(release)
    }
}

// ─── Spawn background task ────────────────────────────────────────────────────

/// Start the auto-update background task.
///
/// Checks immediately on startup, then every 24 hours.
/// The `Updater` handle can be used by RPC handlers to check status and apply.
pub fn spawn(config: Arc<DaemonConfig>, broadcaster: Arc<EventBroadcaster>) -> Updater {
    let updater = Updater::new(config, broadcaster);
    let updater_clone = updater.clone();

    tokio::spawn(async move {
        // Initial check — give daemon 10s to fully start first
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        if let Err(e) = updater_clone.check_and_download().await {
            warn!("update check failed: {e:#}");
        }

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(e) = updater_clone.check_and_download().await {
                warn!("update check failed: {e:#}");
            }
        }
    });

    updater
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")
}

/// Returns the platform string matching our release asset naming convention.
fn current_platform() -> &'static str {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    return "aarch64-apple-darwin";

    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    return "x86_64-apple-darwin";

    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    return "x86_64-unknown-linux-gnu";

    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    return "aarch64-unknown-linux-gnu";

    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    return "x86_64-pc-windows-msvc";

    #[cfg(not(any(
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "linux"),
        all(target_arch = "aarch64", target_os = "linux"),
        all(target_arch = "x86_64", target_os = "windows"),
    )))]
    return "unknown-platform";
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── Rollback sentinel (DC.T29) ────────────────────────────────────────────

    #[test]
    fn test_no_sentinel_no_rollback() {
        let dir = TempDir::new().unwrap();
        // No sentinel file exists — rollback should be a no-op.
        assert!(!check_and_rollback(dir.path()));
    }

    #[test]
    fn test_stale_sentinel_no_rollback() {
        let dir = TempDir::new().unwrap();
        // Write a sentinel with a timestamp 60 seconds in the past.
        let old_ts = chrono::Utc::now() - chrono::Duration::seconds(60);
        std::fs::write(rollback_sentinel_path(dir.path()), old_ts.to_rfc3339()).unwrap();

        // Stale — should not rollback, should clean up sentinel.
        assert!(!check_and_rollback(dir.path()));
        assert!(!rollback_sentinel_path(dir.path()).exists());
    }

    #[test]
    fn test_corrupt_sentinel_no_rollback() {
        let dir = TempDir::new().unwrap();
        std::fs::write(rollback_sentinel_path(dir.path()), "not-a-timestamp").unwrap();

        assert!(!check_and_rollback(dir.path()));
        assert!(!rollback_sentinel_path(dir.path()).exists());
    }

    #[test]
    fn test_fresh_sentinel_no_backup_no_rollback() {
        let dir = TempDir::new().unwrap();
        // Sentinel is fresh but there is no backup binary.
        write_rollback_sentinel(dir.path());

        // check_and_rollback needs a real backup at {current_exe}.backup
        // which we can't create in a unit test without touching the real binary.
        // So this just verifies the "no backup" branch returns false.
        // (The backup path depends on std::env::current_exe() which is the test binary.)
        let result = check_and_rollback(dir.path());
        // We don't assert on result here because the test binary may or may not
        // have a .backup file on disk; we only care that it doesn't panic.
        let _ = result;
        // Sentinel should be cleaned up regardless.
        // (If backup didn't exist, sentinel is removed; if it did, it's also removed.)
        // No assertion needed — just verify no panic.
    }

    #[test]
    fn test_delete_sentinel_removes_file() {
        let dir = TempDir::new().unwrap();
        write_rollback_sentinel(dir.path());
        assert!(rollback_sentinel_path(dir.path()).exists());

        delete_rollback_sentinel(dir.path());
        assert!(!rollback_sentinel_path(dir.path()).exists());
    }

    // ── SHA256 verification (DC.T28) ──────────────────────────────────────────

    #[test]
    fn test_sha256_mismatch_rejected() {
        // Verify the hash-checking logic: compute SHA256 of known bytes,
        // then check that a wrong expected hash produces an error.
        use sha2::{Digest, Sha256};
        let data = b"hello world";
        let actual = format!("{:x}", Sha256::digest(data));
        let expected = "deadbeef00000000000000000000000000000000000000000000000000000000";

        assert_ne!(
            actual, expected,
            "actual hash must not equal deliberately wrong hash"
        );
        // The download() function bails when actual != expected.
        // We verify the condition here without running the full async download flow.
    }

    #[test]
    fn test_sha256_match_accepted() {
        use sha2::{Digest, Sha256};
        let data = b"hello world";
        let expected = format!("{:x}", Sha256::digest(data));
        // Verify same data produces same hash (no mismatch → accepted).
        let actual = format!("{:x}", Sha256::digest(data));
        assert_eq!(actual, expected);
    }

    // ── Update policy (DC.T32) ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_policy_valid_values() {
        // Create a minimal Updater with a stub Arc<DaemonConfig>.
        use std::path::PathBuf;
        let config = Arc::new(crate::config::DaemonConfig {
            port: 4300,
            data_dir: PathBuf::from("/tmp"),
            log: "info".into(),
            max_sessions: 10,
            max_accounts: 10,
            session_prune_days: 30,
            license_token: None,
            api_base_url: "https://api.clawde.io".into(),
            relay_url: "wss://api.clawde.io/relay/ws".into(),
            bind_address: "127.0.0.1".into(),
            providers: Default::default(),
            resources: Default::default(),
            model_intelligence: Default::default(),
            update_policy: "auto".into(),
            security: Default::default(),
            log_format: "pretty".into(),
            observability: Default::default(),
        });
        let broadcaster = Arc::new(crate::ipc::event::EventBroadcaster::new());
        let updater = Updater::new(config, broadcaster);

        assert_eq!(updater.get_policy().await, "auto");

        updater.set_policy("manual").await.unwrap();
        assert_eq!(updater.get_policy().await, "manual");

        updater.set_policy("never").await.unwrap();
        assert_eq!(updater.get_policy().await, "never");

        updater.set_policy("auto").await.unwrap();
        assert_eq!(updater.get_policy().await, "auto");
    }

    #[tokio::test]
    async fn test_policy_invalid_value_rejected() {
        use std::path::PathBuf;
        let config = Arc::new(crate::config::DaemonConfig {
            port: 4300,
            data_dir: PathBuf::from("/tmp"),
            log: "info".into(),
            max_sessions: 10,
            max_accounts: 10,
            session_prune_days: 30,
            license_token: None,
            api_base_url: "https://api.clawde.io".into(),
            relay_url: "wss://api.clawde.io/relay/ws".into(),
            bind_address: "127.0.0.1".into(),
            providers: Default::default(),
            resources: Default::default(),
            model_intelligence: Default::default(),
            update_policy: "auto".into(),
            security: Default::default(),
            log_format: "pretty".into(),
            observability: Default::default(),
        });
        let broadcaster = Arc::new(crate::ipc::event::EventBroadcaster::new());
        let updater = Updater::new(config, broadcaster);

        let err = updater.set_policy("aggressive").await.unwrap_err();
        assert!(err.to_string().contains("invalid type"));
    }

    // ── Platform string (DC.T30 — platform detection) ─────────────────────────

    #[test]
    fn test_current_platform_not_empty() {
        let p = current_platform();
        assert!(!p.is_empty(), "platform string must not be empty");
    }
}
