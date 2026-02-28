// SPDX-License-Identifier: MIT
// Sprint M: Pack Marketplace — ed25519 pack signing (PK.T05)
//
// PackSigner provides `sign_pack` and `verify_signature` using real
// ed25519 cryptography via `ed25519-dalek`.
//
// Key format: raw 32-byte little-endian scalar (signing) / compressed point
// (verifying), stored hex-encoded in files on disk.

use anyhow::{Context as _, Result};
use std::path::Path;

// ─── PackSigner ───────────────────────────────────────────────────────────────

/// Handles ed25519 pack signing and signature verification.
pub struct PackSigner;

impl PackSigner {
    // ─── Sign ──────────────────────────────────────────────────────────────

    /// Compute an ed25519 signature over the `pack.toml` contents in
    /// `pack_dir` using the private key at `private_key_path`.
    ///
    /// The private key file must contain 64 hex-encoded bytes (the
    /// `SigningKey` scalar — use `PackSigner::generate_keypair` to create
    /// one).
    ///
    /// Returns a 128-character hex-encoded ed25519 signature.
    pub fn sign_pack(pack_dir: &Path, private_key_path: &Path) -> Result<String> {
        use ed25519_dalek::{Signer, SigningKey};

        let content = read_pack_toml(pack_dir)?;
        let key_hex = std::fs::read_to_string(private_key_path).with_context(|| {
            format!("cannot read signing key at {}", private_key_path.display())
        })?;
        let key_bytes =
            decode_hex_32(key_hex.trim()).context("signing key must be 32 hex bytes")?;
        let signing_key = SigningKey::from_bytes(&key_bytes);
        let sig = signing_key.sign(&content);
        Ok(hex::encode(sig.to_bytes()))
    }

    // ─── Verify ────────────────────────────────────────────────────────────

    /// Verify that `signature` (128 hex chars) is a valid ed25519 signature
    /// over the `pack.toml` in `pack_dir`, produced by the key whose
    /// public half is `public_key` (64 hex-encoded bytes).
    ///
    /// Returns `Ok(true)` if valid, `Ok(false)` if signature doesn't match.
    pub fn verify_signature(pack_dir: &Path, signature: &str, public_key: &str) -> Result<bool> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        let content = read_pack_toml(pack_dir)?;
        let pub_bytes =
            decode_hex_32(public_key.trim()).context("public key must be 32 hex bytes")?;
        let verifying_key =
            VerifyingKey::from_bytes(&pub_bytes).context("invalid ed25519 public key")?;
        let sig_bytes = decode_hex_64(signature.trim())
            .context("signature must be 64 hex bytes (128 hex chars)")?;
        let sig = Signature::from_bytes(&sig_bytes);
        Ok(verifying_key.verify(&content, &sig).is_ok())
    }

    // ─── Key generation ────────────────────────────────────────────────────

    /// Generate a fresh ed25519 keypair.
    ///
    /// Returns `(private_key_hex, public_key_hex)` — both 32-byte scalars
    /// hex-encoded.  Write each to a file and keep the private key secret.
    pub fn generate_keypair() -> (String, String) {
        use ed25519_dalek::SigningKey;
        use rand_core::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        (
            hex::encode(signing_key.to_bytes()),
            hex::encode(verifying_key.to_bytes()),
        )
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn read_pack_toml(pack_dir: &Path) -> Result<Vec<u8>> {
    let path = pack_dir.join("pack.toml");
    std::fs::read(&path).with_context(|| format!("cannot read pack.toml at {}", path.display()))
}

fn decode_hex_32(s: &str) -> Result<[u8; 32]> {
    let bytes = hex_decode(s)?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 32 bytes, got {}", s.len() / 2))
}

fn decode_hex_64(s: &str) -> Result<[u8; 64]> {
    let bytes = hex_decode(s)?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 64 bytes, got {}", s.len() / 2))
}

fn hex_decode(s: &str) -> Result<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(s.get(i..i + 2).unwrap_or("xx"), 16)
                .map_err(|e| anyhow::anyhow!("hex decode error at byte {i}: {e}"))
        })
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_pack_toml(dir: &TempDir, content: &str) {
        std::fs::write(dir.path().join("pack.toml"), content).unwrap();
    }

    fn write_key_file(dir: &TempDir, name: &str, hex: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, hex).unwrap();
        path
    }

    #[test]
    fn generate_keypair_produces_distinct_keys() {
        let (priv1, pub1) = PackSigner::generate_keypair();
        let (priv2, pub2) = PackSigner::generate_keypair();
        assert_ne!(priv1, priv2, "two keypairs should differ");
        assert_ne!(pub1, pub2);
        assert_eq!(priv1.len(), 64); // 32 bytes hex
        assert_eq!(pub1.len(), 64);
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let (priv_hex, pub_hex) = PackSigner::generate_keypair();
        let dir = TempDir::new().unwrap();
        write_pack_toml(&dir, "[pack]\nname = \"test\"\nversion = \"1.0.0\"");
        let key_file = write_key_file(&dir, "signing.key", &priv_hex);

        let sig = PackSigner::sign_pack(dir.path(), &key_file).unwrap();
        assert_eq!(
            sig.len(),
            128,
            "ed25519 signature is 64 bytes = 128 hex chars"
        );
        assert!(PackSigner::verify_signature(dir.path(), &sig, &pub_hex).unwrap());
    }

    #[test]
    fn verify_rejects_wrong_public_key() {
        let (priv_hex, _) = PackSigner::generate_keypair();
        let (_, other_pub) = PackSigner::generate_keypair();
        let dir = TempDir::new().unwrap();
        write_pack_toml(&dir, "[pack]\nname = \"bad\"\nversion = \"0.1.0\"");
        let key_file = write_key_file(&dir, "signing.key", &priv_hex);

        let sig = PackSigner::sign_pack(dir.path(), &key_file).unwrap();
        let valid = PackSigner::verify_signature(dir.path(), &sig, &other_pub).unwrap();
        assert!(!valid, "wrong public key must not verify");
    }

    #[test]
    fn verify_rejects_tampered_pack_toml() {
        let (priv_hex, pub_hex) = PackSigner::generate_keypair();
        let dir = TempDir::new().unwrap();
        write_pack_toml(&dir, "[pack]\nname = \"original\"\nversion = \"1.0.0\"");
        let key_file = write_key_file(&dir, "signing.key", &priv_hex);

        let sig = PackSigner::sign_pack(dir.path(), &key_file).unwrap();
        // Tamper with pack.toml after signing
        write_pack_toml(&dir, "[pack]\nname = \"tampered\"\nversion = \"1.0.0\"");
        let valid = PackSigner::verify_signature(dir.path(), &sig, &pub_hex).unwrap();
        assert!(!valid, "tampered pack.toml must not verify");
    }

    #[test]
    fn verify_rejects_malformed_signature() {
        let (_, pub_hex) = PackSigner::generate_keypair();
        let dir = TempDir::new().unwrap();
        write_pack_toml(&dir, "[pack]\nname = \"x\"\nversion = \"0.1.0\"");

        let result = PackSigner::verify_signature(dir.path(), "not-hex-at-all", &pub_hex);
        assert!(result.is_err(), "malformed sig should error");
    }

    #[test]
    fn sign_errors_on_missing_pack_toml() {
        let dir = TempDir::new().unwrap();
        let key_file = write_key_file(&dir, "signing.key", &"aa".repeat(32));
        let result = PackSigner::sign_pack(dir.path(), &key_file);
        assert!(result.is_err());
    }
}
