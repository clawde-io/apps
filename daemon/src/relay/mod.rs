//! Outbound relay client — connects to the ClawDE relay server so remote
//! Flutter clients can reach the daemon over the internet.
//!
//! Only started when the license cache has `features.relay == true`.
//!
//! Protocol:
//! 1. Connect to `CLAWD_RELAY_URL` (default: `wss://api.clawde.io/relay/ws`)
//! 2. Send `{ "type": "register", "daemonId": "...", "token": "..." }`
//! 3. Receive RPC frames from relay; dispatch through local IPC handler; reply
//! 4. Forward daemon push events (broadcaster) back to relay → remote client
//! 5. On disconnect: reconnect with exponential backoff (2s → 4s → 8s … max 60s)

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::config::DaemonConfig;
use crate::license::LicenseInfo;
use crate::AppContext;

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

                // Outbound channel — receives both RPC responses (from handle_inbound)
                // and daemon push events (from the broadcaster forwarder below).
                let (out_tx, mut out_rx) = mpsc::channel::<String>(128);
                let bcast_tx = out_tx.clone();

                tokio::select! {
                    _ = handle_inbound(&mut stream, &ctx, out_tx) => {
                        warn!("relay: inbound stream closed");
                    }
                    _ = handle_outbound(&mut out_rx, &mut sink) => {
                        warn!("relay: outbound sink closed");
                    }
                    _ = forward_broadcasts(&ctx, bcast_tx) => {
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

/// Receive RPC frames from the relay, dispatch them through the local IPC handler,
/// and send the response back via `out_tx`.
async fn handle_inbound(
    stream: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    ctx: &Arc<AppContext>,
    out_tx: mpsc::Sender<String>,
) {
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("relay: inbound frame ({} bytes)", text.len());
                let response = crate::ipc::dispatch_text(&text, ctx).await;
                let _ = out_tx.send(response).await;
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
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
async fn forward_broadcasts(ctx: &Arc<AppContext>, tx: mpsc::Sender<String>) {
    let mut rx = ctx.broadcaster.subscribe();
    loop {
        match rx.recv().await {
            Ok(json) => {
                if tx.send(json).await.is_err() {
                    break;
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
