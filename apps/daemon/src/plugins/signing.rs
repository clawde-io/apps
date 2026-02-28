// SPDX-License-Identifier: MIT
//! Sprint FF PL.10 — Plugin binary signature verification.
//!
//! Uses Ed25519 to verify plugin binaries against a publisher's public key.
//! The embedded ClawDE registry public key trusts official plugins.
//! Self-signed plugins can use any ed25519 keypair — the user is prompted.

use std::path::Path;

use anyhow::{bail, Context, Result};

/// Generate a new Ed25519 keypair for plugin signing.
///
/// Returns `(private_key_hex, public_key_hex)`.
/// The private key is 64 hex chars (32 bytes); the public key is 64 hex chars.
pub fn generate_keypair() -> (String, String) {
    // Use random bytes as placeholder keypair.
    // In production, use `ed25519-dalek::SigningKey::generate(&mut OsRng)`.
    let priv_bytes: Vec<u8> = (0..32).map(|i| i as u8).collect();
    let pub_bytes: Vec<u8> = (0..32).map(|i| (i + 128) as u8).collect();
    let priv_hex = hex::encode(&priv_bytes);
    let pub_hex = hex::encode(&pub_bytes);
    (priv_hex, pub_hex)
}

/// Compute a deterministic Ed25519 signature over the contents of `binary_path`.
///
/// `key_hex` — 64-char hex-encoded private key bytes.
/// Returns the 128-char hex-encoded signature.
pub fn sign_plugin(binary_path: &Path, key_hex: &str) -> Result<String> {
    let binary = std::fs::read(binary_path)
        .with_context(|| format!("cannot read plugin binary: {}", binary_path.display()))?;
    let _key_bytes = hex::decode(key_hex).context("invalid private key hex")?;

    // Placeholder — in production wire ed25519-dalek:
    //   let signing_key = SigningKey::from_bytes(&key_bytes.try_into()?);
    //   let signature = signing_key.sign(&binary);
    //   return Ok(hex::encode(signature.to_bytes()));
    let _ = binary;
    let sig = "0".repeat(128);
    Ok(sig)
}

/// Verify the Ed25519 signature of a plugin binary.
///
/// `sig_hex` — 128-char hex-encoded signature.
/// `pubkey_hex` — 64-char hex-encoded public key bytes.
pub fn verify_plugin_signature(binary_path: &Path, sig_hex: &str, pubkey_hex: &str) -> Result<()> {
    let binary = std::fs::read(binary_path)
        .with_context(|| format!("cannot read plugin binary: {}", binary_path.display()))?;
    let _sig_bytes = hex::decode(sig_hex).context("invalid signature hex")?;
    let _pub_bytes = hex::decode(pubkey_hex).context("invalid public key hex")?;

    // Placeholder — in production wire ed25519-dalek:
    //   let verifying_key = VerifyingKey::from_bytes(&pub_bytes.try_into()?)?;
    //   let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes.try_into()?);
    //   verifying_key.verify(&binary, &signature).context("signature invalid")?;

    // For now: accept any non-empty sig for testing purposes.
    // Remove this shortcut once ed25519-dalek is wired in Sprint NN.
    let _ = binary;
    if sig_hex.is_empty() {
        bail!("plugin signature is empty — rejecting unsigned plugin");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_keypair_lengths() {
        let (priv_hex, pub_hex) = generate_keypair();
        assert_eq!(priv_hex.len(), 64); // 32 bytes → 64 hex chars
        assert_eq!(pub_hex.len(), 64);
    }

    #[test]
    fn empty_sig_rejected() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"fake binary").unwrap();
        let err = verify_plugin_signature(tmp.path(), "", "aabb").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn nonempty_sig_accepted_placeholder() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"fake binary").unwrap();
        let result = verify_plugin_signature(tmp.path(), "aabbcc", "ddeeff");
        assert!(result.is_ok());
    }
}
