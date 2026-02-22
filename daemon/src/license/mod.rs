//! License verification against the ClawDE backend.
//!
//! On startup the daemon calls POST /daemon/verify with its `daemon_id` and
//! `daemonVersion` in the Authorization Bearer header (user JWT).
//!
//! The response `{ tier, features: { relay, autoSwitch } }` is cached in
//! SQLite for up to 7 days.  If verification fails and a valid cache exists
//! the cached values are used (offline grace period).

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::config::DaemonConfig;
use crate::storage::Storage;

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    pub relay: bool,
    pub auto_switch: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LicenseInfo {
    pub tier: String,
    pub features: Features,
}

impl LicenseInfo {
    pub fn free() -> Self {
        Self {
            tier: "free".to_string(),
            features: Features::default(),
        }
    }
}

// ─── API types (deserialize response) ────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerifyResponse {
    tier: String,
    features: Features,
}

// ─── Verification ─────────────────────────────────────────────────────────────

/// Calls POST /daemon/verify.  On success caches the result.
/// On failure returns cached data if within grace period, else returns Free.
pub async fn verify_and_cache(
    storage: &Storage,
    config: &DaemonConfig,
    daemon_id: &str,
) -> LicenseInfo {
    // Skip verification if no token configured.
    let token = match &config.license_token {
        Some(t) if !t.is_empty() => t.clone(),
        _ => {
            info!("no license token configured — running as Free tier");
            return LicenseInfo::free();
        }
    };

    match call_verify(config, daemon_id, &token).await {
        Ok(info) => {
            if let Err(e) = write_cache(storage, &info).await {
                warn!("failed to write license cache: {e:#}");
            }
            info!(tier = %info.tier, "license verified");
            info
        }
        Err(e) => {
            warn!("license verify failed: {e:#} — checking cache");
            read_cache_grace(storage).await
        }
    }
}

/// Returns cached license info if it is within the 7-day grace period,
/// otherwise returns Free.
pub async fn get_cached(storage: &Storage) -> LicenseInfo {
    read_cache_grace(storage).await
}

// ─── Private helpers ──────────────────────────────────────────────────────────

async fn call_verify(
    config: &DaemonConfig,
    daemon_id: &str,
    token: &str,
) -> Result<LicenseInfo> {
    let url = format!("{}/daemon/verify", config.api_base_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({
            "daemonId": daemon_id,
            "daemonVersion": env!("CARGO_PKG_VERSION"),
        }))
        .send()
        .await?
        .error_for_status()?;

    let body: VerifyResponse = resp.json().await?;
    Ok(LicenseInfo {
        tier: body.tier,
        features: body.features,
    })
}

async fn write_cache(storage: &Storage, info: &LicenseInfo) -> Result<()> {
    let now = Utc::now();
    let valid_until = now + Duration::days(7);
    let features_json = serde_json::to_string(&info.features)?;
    storage
        .set_license_cache(
            &info.tier,
            &features_json,
            &now.to_rfc3339(),
            &valid_until.to_rfc3339(),
        )
        .await
}

async fn read_cache_grace(storage: &Storage) -> LicenseInfo {
    match storage.get_license_cache().await {
        Ok(Some(row)) => {
            // Check if within grace period.
            match DateTime::parse_from_rfc3339(&row.valid_until) {
                Ok(valid_until) if Utc::now() < valid_until.with_timezone(&Utc) => {
                    let features: Features =
                        serde_json::from_str(&row.features).unwrap_or_default();
                    info!(tier = %row.tier, "using cached license (grace period)");
                    LicenseInfo {
                        tier: row.tier,
                        features,
                    }
                }
                _ => {
                    warn!("cached license expired — falling back to Free");
                    LicenseInfo::free()
                }
            }
        }
        Ok(None) => {
            info!("no license cache — using Free tier");
            LicenseInfo::free()
        }
        Err(e) => {
            warn!("failed to read license cache: {e:#}");
            LicenseInfo::free()
        }
    }
}
