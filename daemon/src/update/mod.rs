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
//! RPC surface:
//! - `daemon.checkUpdate`  → returns `{ current, latest, available }` (no download)
//! - `daemon.applyUpdate`  → applies staged update immediately (user-triggered)
//!
//! Broadcasts:
//! - `daemon.updateAvailable { current, latest, releaseNotesUrl }`
//! - `daemon.updating        { version }`
//! - `daemon.updateFailed    { version, reason }`

use std::path::PathBuf;
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

// ─── Updater ─────────────────────────────────────────────────────────────────

/// Manages the self-update lifecycle.
#[derive(Clone)]
pub struct Updater {
    config: Arc<DaemonConfig>,
    broadcaster: Arc<EventBroadcaster>,
    pending: Arc<Mutex<Option<PendingUpdate>>>,
}

impl Updater {
    pub fn new(config: Arc<DaemonConfig>, broadcaster: Arc<EventBroadcaster>) -> Self {
        Self {
            config,
            broadcaster,
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// Check GitHub Releases for a newer version.
    /// Returns `(current_version, latest_version, is_newer)`.
    pub async fn check(&self) -> Result<(String, String, bool)> {
        let current = Version::parse(env!("CARGO_PKG_VERSION"))
            .context("invalid CARGO_PKG_VERSION")?;

        let release = self.fetch_latest_release().await?;
        let tag = release.tag_name.trim_start_matches('v').to_string();
        let latest = Version::parse(&tag).context("invalid release tag semver")?;

        let is_newer = latest > current;
        Ok((current.to_string(), latest.to_string(), is_newer))
    }

    /// Check + broadcast `daemon.updateAvailable` if newer, then download.
    /// Called on startup and every 24 hours.
    pub async fn check_and_download(&self) -> Result<()> {
        let (current, latest, is_newer) = self.check().await?;

        if !is_newer {
            debug!(current = %current, "no update available");
            return Ok(());
        }

        info!(current = %current, latest = %latest, "update available — downloading");

        let release = self.fetch_latest_release().await?;
        let release_notes_url = release.html_url.clone();

        self.broadcaster.broadcast(
            "daemon.updateAvailable",
            json!({
                "current": current,
                "latest": latest,
                "releaseNotesUrl": release_notes_url,
            }),
        );

        // Download in background — don't block startup
        let this = self.clone();
        tokio::spawn(async move {
            if let Err(e) = this.download(&release, &latest).await {
                warn!("update download failed: {e:#}");
                this.broadcaster.broadcast(
                    "daemon.updateFailed",
                    json!({ "version": latest, "reason": e.to_string() }),
                );
            }
        });

        Ok(())
    }

    /// Download the release binary and verify SHA256.
    async fn download(&self, release: &GhRelease, version: &str) -> Result<()> {
        let platform = current_platform();
        debug!(platform = %platform, "looking for release asset");

        // Find matching binary asset (e.g. "clawd-aarch64-apple-darwin")
        let binary_asset = release
            .assets
            .iter()
            .find(|a| a.name == format!("clawd-{platform}"))
            .with_context(|| format!("no asset for platform {platform}"))?;

        // Find matching checksum asset
        let checksum_asset = release
            .assets
            .iter()
            .find(|a| a.name == format!("clawd-{platform}.sha256"))
            .with_context(|| format!("no checksum asset for platform {platform}"))?;

        // Fetch expected checksum
        let client = build_client()?;
        let checksum_text = client
            .get(&checksum_asset.browser_download_url)
            .send()
            .await?
            .text()
            .await?;
        let expected_hash = checksum_text.split_whitespace().next().unwrap_or("").to_string();

        // Download binary with streaming to avoid loading entire file into RAM
        let dest = self
            .config
            .data_dir
            .join(format!("clawd-update-{version}"));

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
            bail!(
                "SHA256 mismatch: expected {expected_hash}, got {actual_hash}"
            );
        }

        // Make executable on Unix
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
    /// Renames the current binary to `clawd-backup`, renames the staged update
    /// to the current binary path, then exec's the new binary.
    /// On Windows, issues a service restart instead.
    pub async fn apply_if_ready(&self) -> Result<bool> {
        let pending = {
            let guard = self.pending.lock().await;
            guard.clone()
        };

        let Some(update) = pending else {
            return Ok(false);
        };

        self.broadcaster.broadcast(
            "daemon.updating",
            json!({ "version": update.version }),
        );

        info!(version = %update.version, "applying update");

        let current_exe = std::env::current_exe().context("failed to get current binary path")?;
        let backup = current_exe.with_extension("backup");

        // Rename current → backup
        std::fs::rename(&current_exe, &backup)
            .context("failed to rename current binary to backup")?;

        // Rename staged → current
        std::fs::rename(&update.binary_path, &current_exe)
            .context("failed to rename update binary")?;

        // Exec the new binary (Unix) or restart (Windows)
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let args: Vec<String> = std::env::args().collect();
            // exec() replaces the process on success; returns io::Error on failure
            let err = std::process::Command::new(&current_exe)
                .args(&args[1..])
                .exec();
            return Err(anyhow::anyhow!("exec failed: {err}"));
        }

        #[cfg(not(unix))]
        {
            // On Windows, restart the service via sc.exe
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

        // Repeat every 24 hours
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        interval.tick().await; // consume first immediate tick
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
/// E.g. `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`.
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
