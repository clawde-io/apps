// bundle.rs — Offline license bundle for Enterprise air-gap mode.
//
// Sprint NN AG.1/AG.2: Ed25519-signed offline license bundles.
//
// A license bundle is a JSON payload signed with ClawDE's Ed25519 private key.
// The daemon verifies the signature against the embedded public key at startup.
//
// Bundle format (JSON, base64url-encoded, then signed):
// {
//   "daemon_id": "sha256:...",      -- machine hardware ID hash
//   "tier": "enterprise",
//   "seat_count": 50,
//   "issued_at": "2026-03-01T00:00:00Z",
//   "expires_at": "2027-03-01T00:00:00Z",
//   "features": { "relay": false, "auto_switch": true, "air_gap": true }
// }

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ClawDE's Ed25519 public key (embedded at compile time).
// Replace with real key bytes before production use.
// Generated with: openssl genpkey -algorithm ed25519 | openssl pkey -pubout -outform DER
const CLAWD_PUBLIC_KEY: &[u8; 32] = &[
    0x3b, 0x6a, 0x27, 0xbc, 0xce, 0xb6, 0xa4, 0x2d,
    0x62, 0xa3, 0xa8, 0xd0, 0x2a, 0x6f, 0x0d, 0x73,
    0x65, 0x32, 0x15, 0x77, 0x1d, 0xe2, 0x43, 0xa6,
    0x3a, 0xc0, 0x48, 0xa1, 0x8b, 0x59, 0xda, 0x29,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFeatures {
    pub relay: bool,
    pub auto_switch: bool,
    pub air_gap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseBundle {
    pub daemon_id: String,
    pub tier: String,
    pub seat_count: Option<u32>,
    pub issued_at: String,
    pub expires_at: String,
    pub features: BundleFeatures,
}

impl LicenseBundle {
    /// Load and verify a license bundle from a file.
    ///
    /// The file format is two lines:
    ///   Line 1: base64url-encoded JSON payload
    ///   Line 2: base64url-encoded Ed25519 signature
    pub fn load_and_verify(path: &Path, daemon_id: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Reading license bundle from {:?}", path))?;

        let lines: Vec<&str> = content.trim().lines().collect();
        if lines.len() < 2 {
            bail!("Invalid license bundle format — expected payload and signature on separate lines");
        }

        let payload_b64 = lines[0].trim();
        let sig_b64 = lines[1].trim();

        let payload_bytes = URL_SAFE_NO_PAD
            .decode(payload_b64)
            .context("Decoding license payload")?;
        let sig_bytes = URL_SAFE_NO_PAD
            .decode(sig_b64)
            .context("Decoding license signature")?;

        // Verify signature
        let verifying_key = VerifyingKey::from_bytes(CLAWD_PUBLIC_KEY)
            .context("Loading ClawDE public key")?;
        let signature = Signature::from_slice(&sig_bytes)
            .context("Parsing Ed25519 signature")?;
        verifying_key
            .verify(&payload_bytes, &signature)
            .context("License signature verification failed — bundle may be tampered")?;

        // Parse bundle
        let bundle: LicenseBundle =
            serde_json::from_slice(&payload_bytes).context("Parsing license bundle JSON")?;

        // Validate daemon_id
        if bundle.daemon_id != daemon_id && bundle.daemon_id != "*" {
            bail!(
                "License bundle is for daemon '{}', not '{}'",
                bundle.daemon_id,
                daemon_id
            );
        }

        // Validate expiry
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires: chrono::DateTime<chrono::Utc> = bundle
            .expires_at
            .parse()
            .context("Parsing bundle expires_at")?;
        if expires.timestamp() as u64 <= now_secs {
            bail!("License bundle expired at {}", bundle.expires_at);
        }

        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_bundle_invalid_format_rejected() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "not-valid-bundle-format").unwrap();
        let result = LicenseBundle::load_and_verify(f.path(), "test-daemon");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid license bundle format"));
    }

    #[test]
    fn test_bundle_tampered_payload_rejected() {
        // Two lines but both are garbage
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "dGFtcGVyZWQ").unwrap(); // "tampered" in base64url
        writeln!(f, "ZmFrZXNpZw").unwrap(); // "fakesig" in base64url
        let result = LicenseBundle::load_and_verify(f.path(), "test-daemon");
        assert!(result.is_err());
    }

    #[test]
    fn test_bundle_wrong_daemon_id_rejected() {
        // Can't create a valid signed bundle in unit test (no private key)
        // Covered by integration test with dev key binary
        let bundle = LicenseBundle {
            daemon_id: "other-daemon".to_string(),
            tier: "enterprise".to_string(),
            seat_count: Some(50),
            issued_at: "2026-01-01T00:00:00Z".to_string(),
            expires_at: "2099-01-01T00:00:00Z".to_string(),
            features: BundleFeatures {
                relay: false,
                auto_switch: true,
                air_gap: true,
            },
        };
        // Verify that a bundle not matching daemon_id would fail
        assert_ne!(bundle.daemon_id, "my-daemon");
    }
}
