//! Outbound relay client — connects to the ClawDE relay server so remote
//! Flutter clients can reach the daemon over the internet.
//!
//! Only started when the license cache has `features.relay == true`.
//!
//! Protocol:
//! 1. Connect to `CLAWD_RELAY_URL` (default: `wss://api.clawde.io/relay/ws`)
//! 2. Send `{ "type": "register", "daemonId": "...", "token": "..." }`
//! 3. On `client_connected`: reset E2E state, await `e2e_hello` handshake
//! 4. After E2E handshake: decrypt inbound frames, dispatch via local IPC,
//!    encrypt responses before forwarding back through the relay
//! 5. Forward daemon push events (broadcaster) back to relay → remote client
//!    (encrypted when E2E is active)
//! 6. On disconnect: reconnect with exponential backoff (2s → 4s → 8s … max 60s)
//!
//! E2E encryption: X25519 key exchange → HKDF-SHA256 → ChaCha20-Poly1305.
//! See `relay/crypto.rs` for full protocol specification.
//! Backward compatible: clients without E2E support fall back to plaintext
//! (protected by TLS only).

pub mod crypto;

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{debug, info, trace, warn};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::config::DaemonConfig;
use crate::license::LicenseInfo;
use crate::AppContext;

use crypto::RelayE2e;

// ─── Spawn ────────────────────────────────────────────────────────────────────

/// Starts the relay background task if the license allows it.
/// Returns `false` if relay is disabled or token is absent.
pub async fn spawn_if_enabled(
    config: Arc<DaemonConfig>,
    license: &LicenseInfo,
    daemon_id: String,
    ctx: Arc<AppContext>,
) -> bool {
    if !license.features.relay {
        debug!("relay feature disabled — not connecting");
        return false;
    }

    let token = config.license_token.clone().unwrap_or_default();
    if token.is_empty() {
        warn!("relay feature enabled but no license token — skipping relay");
        return false;
    }

    tokio::spawn(relay_loop(config, daemon_id, ctx, token));
    true
}

// ─── Background loop ──────────────────────────────────────────────────────────

async fn relay_loop(
    config: Arc<DaemonConfig>,
    daemon_id: String,
    ctx: Arc<AppContext>,
    token: String,
) {
    let relay_url = config.relay_url.clone();
    let mut backoff_secs: u64 = 2;

    loop {
        info!(url = %relay_url, "relay: connecting");

        match connect_async(&relay_url).await {
            Ok((ws_stream, _)) => {
                info!("relay: connected");
                backoff_secs = 2;

                let (mut sink, mut stream) = ws_stream.split();

                let register_msg = serde_json::json!({
                    "type": "register",
                    "daemonId": daemon_id,
                    "token": token,
                })
                .to_string();

                if let Err(e) = sink.send(Message::Text(register_msg)).await {
                    warn!("relay: failed to send register: {e:#}");
                    sleep_backoff(&mut backoff_secs).await;
                    continue;
                }

                // Shared E2E state between inbound handler and broadcast forwarder.
                let e2e: Arc<Mutex<Option<RelayE2e>>> = Arc::new(Mutex::new(None));
                let e2e_bcast = e2e.clone();

                // Outbound channel — RPC responses (from handle_inbound) and daemon
                // push events (from forward_broadcasts) share this channel.
                let (out_tx, mut out_rx) = mpsc::channel::<String>(128);
                let bcast_tx = out_tx.clone();

                tokio::select! {
                    _ = handle_inbound(&mut stream, &ctx, out_tx, e2e) => {
                        warn!("relay: inbound stream closed");
                    }
                    _ = handle_outbound(&mut out_rx, &mut sink) => {
                        warn!("relay: outbound sink closed");
                    }
                    _ = forward_broadcasts(&ctx, bcast_tx, e2e_bcast) => {
                        warn!("relay: broadcast forwarder stopped");
                    }
                }
            }
            Err(e) => {
                warn!("relay: connection failed: {e:#}");
            }
        }

        sleep_backoff(&mut backoff_secs).await;
    }
}

/// Receive frames from the relay.  Handles E2E handshake, decryption, and
/// dispatch through the local IPC handler.  Encrypts responses before sending.
///
/// Security: No RPC frames are dispatched until the E2E session is fully
/// established. Premature `e2e` frames (before handshake) and any
/// unrecognized frame types are rejected immediately.
async fn handle_inbound(
    stream: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
         + Unpin),
    ctx: &Arc<AppContext>,
    out_tx: mpsc::Sender<String>,
    e2e: Arc<Mutex<Option<RelayE2e>>>,
) {
    // Track whether E2E has been established at least once for this client.
    // This prevents dispatching RPC before the handshake completes.
    let mut e2e_established = false;

    while let Some(msg) = stream.next().await {
        let text = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) | Err(_) => break,
            _ => continue,
        };

        // Parse the outer frame to check its type.
        let frame: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                warn!("relay: unparseable frame: {e}");
                continue;
            }
        };

        let msg_type = frame["type"].as_str().unwrap_or("");

        match msg_type {
            // ── Relay management messages — no IPC dispatch, no response ──────
            "registered" | "client_disconnected" => {
                debug!("relay: ← {msg_type}");
            }

            // ── New client connected — reset E2E state ────────────────────────
            "client_connected" => {
                debug!("relay: ← client_connected — resetting E2E state");
                *e2e.lock().await = None;
                e2e_established = false;
            }

            // ── E2E handshake — client sends its X25519 public key ────────────
            "e2e_hello" => {
                if let Some(client_pubkey) = frame["pubkey"].as_str() {
                    match RelayE2e::server_handshake(client_pubkey) {
                        Ok((server_pubkey, new_e2e)) => {
                            // Send server hello UNENCRYPTED — client needs our
                            // pubkey to derive the shared key.
                            let hello = serde_json::json!({
                                "type": "e2e_hello",
                                "pubkey": server_pubkey,
                            })
                            .to_string();
                            if out_tx.send(hello).await.is_err() {
                                break;
                            }
                            // Activate E2E AFTER sending the hello.
                            *e2e.lock().await = Some(new_e2e);
                            e2e_established = true;
                            info!("relay: E2E encryption established");
                        }
                        Err(e) => warn!("relay: E2E handshake failed: {e:#}"),
                    }
                }
            }

            // ── Encrypted frame from client ───────────────────────────────────
            "e2e" => {
                // Reject RPC frames that arrive before the E2E handshake completes.
                if !e2e_established {
                    warn!("relay: rejecting e2e frame — E2E handshake not yet completed");
                    continue;
                }

                if let Some(payload) = frame["payload"].as_str() {
                    // Decrypt.
                    let inner = {
                        let guard = e2e.lock().await;
                        match guard.as_ref() {
                            Some(state) => match state.decrypt(payload) {
                                Ok(s) => s,
                                Err(e) => {
                                    warn!("relay: E2E decrypt failed: {e:#}");
                                    continue;
                                }
                            },
                            None => {
                                warn!("relay: received e2e frame but E2E session was reset");
                                continue;
                            }
                        }
                    };

                    trace!("relay: inbound e2e frame ({} bytes decrypted)", inner.len());
                    // Relay connections pass "" — they have relay-layer auth, not bearer token.
                    let response = crate::ipc::dispatch_text(&inner, ctx, "").await;

                    // Encrypt response.
                    let out = {
                        let guard = e2e.lock().await;
                        match guard.as_ref() {
                            Some(state) => match state.encrypt(&response) {
                                Ok(p) => serde_json::json!({"type":"e2e","payload":p}).to_string(),
                                Err(e) => {
                                    warn!("relay: E2E encrypt response failed: {e:#}");
                                    continue;
                                }
                            },
                            None => {
                                warn!("relay: E2E deactivated before response could be encrypted");
                                continue;
                            }
                        }
                    };

                    if out_tx.send(out).await.is_err() {
                        break;
                    }
                }
            }

            // ── Unrecognized frame type — E2E required, reject ────────────────
            _ => {
                warn!(
                    "relay: unrecognized frame type '{}' — only E2E frames accepted, closing connection",
                    msg_type
                );
                break;
            }
        }
    }
}

/// Drain the outbound channel and send each message to the relay WebSocket.
async fn handle_outbound(
    rx: &mut mpsc::Receiver<String>,
    sink: &mut (impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
) {
    while let Some(msg) = rx.recv().await {
        if sink.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}

/// Subscribe to daemon push events and forward them to the relay.
/// Encrypts events when E2E is active.
async fn forward_broadcasts(
    ctx: &Arc<AppContext>,
    tx: mpsc::Sender<String>,
    e2e: Arc<Mutex<Option<RelayE2e>>>,
) {
    let mut rx = ctx.broadcaster.subscribe();
    loop {
        match rx.recv().await {
            Ok(json) => {
                let msg: Option<String> = {
                    let guard = e2e.lock().await;
                    match guard.as_ref() {
                        Some(state) => match state.encrypt(&json) {
                            Ok(p) => {
                                Some(serde_json::json!({"type":"e2e","payload":p}).to_string())
                            }
                            Err(e) => {
                                warn!("relay: broadcast E2E encrypt failed: {e:#}");
                                None // drop this event rather than leak plaintext
                            }
                        },
                        None => {
                            // E2E not yet established — silently drop push events
                            // to prevent leaking plaintext over the relay.
                            debug!("relay: dropping broadcast — E2E not yet established");
                            None
                        }
                    }
                };
                if let Some(msg) = msg {
                    if tx.send(msg).await.is_err() {
                        break;
                    }
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!(skipped = n, "relay broadcast lagged");
            }
        }
    }
}

async fn sleep_backoff(backoff_secs: &mut u64) {
    info!("relay: reconnecting in {}s", *backoff_secs);
    tokio::time::sleep(std::time::Duration::from_secs(*backoff_secs)).await;
    *backoff_secs = (*backoff_secs * 2).min(60);
}
