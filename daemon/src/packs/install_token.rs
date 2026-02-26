// packs/install_token.rs — Paid pack install token verification (Sprint SS PU.4-5).
//
// Tokens are HMAC-SHA256 signed JWTs issued by api.clawde.io after purchase.
// Format: "{id}:{pack_slug}:{user_id}:{expires_at_iso}:{hmac_hex}"
//
// The daemon:
//   1. Fetches token from api.clawde.io on first paid pack install
//   2. Stores token in ~/.clawd/pack_tokens/{slug}.token
//   3. Verifies signature + expiry on every daemon start
//   4. Renews automatically if < 24h remaining (PU.5)

use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

type HmacSha256 = Hmac<Sha256>;

const RENEWAL_THRESHOLD_SECS: u64 = 24 * 3600; // renew if <24h remaining

// ─── Token type ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InstallToken {
    pub id: String,
    pub pack_slug: String,
    pub user_id: String,
    pub expires_at: SystemTime,
    raw: String,
}

// ─── Verification ─────────────────────────────────────────────────────────────

pub fn verify_token(raw: &str, secret: &[u8]) -> Result<InstallToken> {
    let parts: Vec<&str> = raw.splitn(5, ':').collect();
    if parts.len() != 5 {
        return Err(anyhow!("malformed install token"));
    }

    let (id, pack_slug, user_id, expires_iso, sig_hex) =
        (parts[0], parts[1], parts[2], parts[3], parts[4]);

    // Verify HMAC
    let payload = format!("{id}:{pack_slug}:{user_id}:{expires_iso}");
    let mut mac = HmacSha256::new_from_slice(secret)?;
    mac.update(payload.as_bytes());
    let expected = mac.finalize().into_bytes();

    let sig_bytes = hex::decode(sig_hex).map_err(|_| anyhow!("invalid token signature hex"))?;
    if expected.as_slice() != sig_bytes.as_slice() {
        return Err(anyhow!("install token signature invalid"));
    }

    // Check expiry
    let expires_at = chrono::DateTime::parse_from_rfc3339(expires_iso)
        .map_err(|_| anyhow!("invalid token expiry timestamp"))?;
    let expires_sys: SystemTime = expires_at.into();

    if expires_sys <= SystemTime::now() {
        return Err(anyhow!("install token expired"));
    }

    Ok(InstallToken {
        id: id.to_string(),
        pack_slug: pack_slug.to_string(),
        user_id: user_id.to_string(),
        expires_at: expires_sys,
        raw: raw.to_string(),
    })
}

// ─── Token file storage ───────────────────────────────────────────────────────

fn token_path(data_dir: &Path, pack_slug: &str) -> PathBuf {
    data_dir.join("pack_tokens").join(format!("{}.token", pack_slug))
}

pub fn save_token(data_dir: &Path, token: &InstallToken) -> Result<()> {
    let dir = data_dir.join("pack_tokens");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(token_path(data_dir, &token.pack_slug), &token.raw)?;
    Ok(())
}

pub fn load_token(data_dir: &Path, pack_slug: &str) -> Option<String> {
    std::fs::read_to_string(token_path(data_dir, pack_slug)).ok()
}

// ─── Renewal check ────────────────────────────────────────────────────────────

/// Returns true if the token should be renewed (< 24h remaining).
pub fn needs_renewal(token: &InstallToken) -> bool {
    let remaining = token
        .expires_at
        .duration_since(SystemTime::now())
        .unwrap_or(Duration::ZERO);
    remaining.as_secs() < RENEWAL_THRESHOLD_SECS
}

// ─── Fetch token from API ─────────────────────────────────────────────────────

pub async fn fetch_token(
    api_base_url: &str,
    pack_slug: &str,
    user_id: &str,
    license_token: &str,
) -> Result<String> {
    let url = format!("{api_base_url}/v1/packs/{pack_slug}/install-token");
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(license_token)
        .json(&serde_json::json!({ "user_id": user_id }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("token fetch failed ({status}): {body}"));
    }

    let data: serde_json::Value = resp.json().await?;
    let token = data["token"]
        .as_str()
        .ok_or_else(|| anyhow!("token not in response"))?
        .to_string();

    Ok(token)
}

/// Verify a pack's install token, renewing if needed.
/// Returns Err if pack requires a paid token and none is valid.
pub async fn ensure_pack_token(
    data_dir: &Path,
    pack_slug: &str,
    api_base_url: &str,
    user_id: Option<&str>,
    license_token: Option<&str>,
    token_secret: &[u8],
) -> Result<()> {
    let raw = match load_token(data_dir, pack_slug) {
        Some(r) => r,
        None => {
            return Err(anyhow!(
                "Pack '{pack_slug}' requires a purchase. Visit https://registry.clawde.io/packs/{pack_slug}/purchase"
            ));
        }
    };

    let token = verify_token(&raw, token_secret)?;

    if needs_renewal(&token) {
        info!(pack_slug, "install token expiring soon — renewing");
        if let (Some(uid), Some(lt)) = (user_id, license_token) {
            match fetch_token(api_base_url, pack_slug, uid, lt).await {
                Ok(new_raw) => {
                    if let Ok(new_token) = verify_token(&new_raw, token_secret) {
                        let _ = save_token(data_dir, &new_token);
                    }
                }
                Err(e) => warn!(pack_slug, "token renewal failed: {e} — using existing token"),
            }
        }
    }

    Ok(())
}
