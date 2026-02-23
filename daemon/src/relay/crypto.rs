//! E2E encryption for relay connections.
//!
//! Protocol: X25519 key exchange → HKDF-SHA256 key derivation → ChaCha20-Poly1305 AEAD.
//!
//! Two direction-specific keys are derived from the shared secret so that each
//! direction has an independent cipher and nonce space:
//!   `key_c2d` (info = "clawd-relay-c2d-v1"): client→daemon (daemon decrypts)
//!   `key_d2c` (info = "clawd-relay-d2c-v1"): daemon→client (daemon encrypts)
//!
//! Wire format — handshake (JSON, unencrypted, forwarded opaquely by relay):
//!   `{"type":"e2e_hello","pubkey":"<32-byte X25519 pubkey, base64url-nopad>"}`
//!
//! Wire format — encrypted frames (JSON, forwarded opaquely by relay):
//!   `{"type":"e2e","payload":"<base64url-nopad of: nonce_12 || ciphertext>"}`
//!
//! Nonces are 12-byte big-endian-zero-padded counters (8-byte LE counter,
//! bytes 8-11 = 0).  Counters start at 0 and increment by 1 per frame.
//! Replay protection: a frame with an unexpected nonce counter is rejected.

use anyhow::{anyhow, Context as _, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use hkdf::Hkdf;
use rand_core::OsRng;
use sha2::Sha256;
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use x25519_dalek::{EphemeralSecret, PublicKey};

// ─── Active E2E session ───────────────────────────────────────────────────────

/// Active E2E session state.  Holds two ChaCha20-Poly1305 ciphers (one per
/// direction) and atomic monotonic counters to prevent nonce reuse / replay.
/// AtomicU64 counters allow safe interior mutability even if the struct is
/// accessed from multiple tasks (the relay wraps this in a Mutex, but the
/// atomic counters provide defense-in-depth against accidental aliasing).
pub struct RelayE2e {
    cipher_recv: ChaCha20Poly1305, // client→daemon
    cipher_send: ChaCha20Poly1305, // daemon→client
    send_counter: AtomicU64,
    recv_counter: AtomicU64,
}

impl RelayE2e {
    /// Server-side (daemon) handshake.
    ///
    /// `client_pubkey_b64` — base64url-nopad-encoded 32-byte X25519 public key
    /// sent by the client in its `e2e_hello` message.
    ///
    /// Returns `(server_pubkey_b64, RelayE2e)` on success.  The caller should
    /// send the server pubkey back to the client **before** activating the
    /// returned session (the client needs the pubkey unencrypted to derive its
    /// own key).
    pub fn server_handshake(client_pubkey_b64: &str) -> Result<(String, Self)> {
        let raw = URL_SAFE_NO_PAD
            .decode(client_pubkey_b64)
            .context("invalid client pubkey encoding")?;
        let bytes: [u8; 32] = raw
            .try_into()
            .map_err(|_| anyhow!("client pubkey must be 32 bytes"))?;
        let client_pk = PublicKey::from(bytes);

        let server_sk = EphemeralSecret::random_from_rng(OsRng);
        let server_pk = PublicKey::from(&server_sk);
        let shared = server_sk.diffie_hellman(&client_pk);

        let cipher_recv = derive_cipher(shared.as_bytes(), b"clawd-relay-c2d-v1")?;
        let cipher_send = derive_cipher(shared.as_bytes(), b"clawd-relay-d2c-v1")?;

        Ok((
            URL_SAFE_NO_PAD.encode(server_pk.as_bytes()),
            RelayE2e {
                cipher_recv,
                cipher_send,
                send_counter: AtomicU64::new(0),
                recv_counter: AtomicU64::new(0),
            },
        ))
    }

    /// Decrypt an incoming (client→daemon) payload.
    ///
    /// `payload_b64` = base64url-nopad( nonce_12 || ciphertext )
    pub fn decrypt(&self, payload_b64: &str) -> Result<String> {
        let data = URL_SAFE_NO_PAD
            .decode(payload_b64)
            .context("invalid e2e payload")?;
        if data.len() < 12 {
            return Err(anyhow!("e2e payload too short"));
        }
        let (nonce_bytes, ct) = data.split_at(12);

        // Replay protection: verify the nonce counter is what we expect.
        // The caller holds a Mutex over this struct so load+increment is
        // effectively atomic in the relay context.
        let expected = make_nonce(self.recv_counter.load(SeqCst));
        if nonce_bytes != expected {
            return Err(anyhow!("nonce mismatch — possible replay attack"));
        }
        self.recv_counter.fetch_add(1, SeqCst);

        let pt = self
            .cipher_recv
            .decrypt(Nonce::from_slice(nonce_bytes), ct)
            .map_err(|_| anyhow!("AEAD decrypt failed"))?;
        String::from_utf8(pt).context("decrypted bytes are not valid UTF-8")
    }

    /// Encrypt an outgoing (daemon→client) frame.
    ///
    /// Returns base64url-nopad( nonce_12 || ciphertext ).
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        // fetch_add returns the old value, which is the nonce counter to use.
        let counter = self.send_counter.fetch_add(1, SeqCst);
        let nonce_bytes = make_nonce(counter);

        let ct = self
            .cipher_send
            .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_bytes())
            .map_err(|_| anyhow!("AEAD encrypt failed"))?;

        let mut payload = nonce_bytes.to_vec();
        payload.extend_from_slice(&ct);
        Ok(URL_SAFE_NO_PAD.encode(payload))
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn derive_cipher(ikm: &[u8], info: &[u8]) -> Result<ChaCha20Poly1305> {
    let hk = Hkdf::<Sha256>::new(None, ikm);
    let mut okm = [0u8; 32];
    hk.expand(info, &mut okm).map_err(|_| anyhow!("HKDF expand failed"))?;
    Ok(ChaCha20Poly1305::new(Key::from_slice(&okm)))
}

fn make_nonce(counter: u64) -> [u8; 12] {
    let mut bytes = [0u8; 12];
    bytes[..8].copy_from_slice(&counter.to_le_bytes());
    bytes
}
