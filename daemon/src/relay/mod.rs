//! Outbound relay client — connects to the ClawDE relay server so remote
//! Flutter clients can reach the daemon over the internet.
//!
//! Only started when the license cache has `features.relay == true`.
//!
//! Protocol:
//! 1. Connect to `CLAWD_RELAY_URL` (default: `wss://api.clawde.io/relay/ws`)
//! 2. Send `{ "type": "register", "daemonId": "...", "token": "..." }`
//! 3. Receive frames from relay; forward them to the local IPC broadcaster
//! 4. On disconnect: reconnect with exponential backoff (2s → 4s → 8s … max 60s)

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::config::DaemonConfig;
use crate::ipc::event::EventBroadcaster;
use crate::license::LicenseInfo;

// ─── Relay client handle ──────────────────────────────────────────────────────

/// Handle used to forward messages through the relay to remote clients.
#[derive(Clone)]
pub struct RelayClient {
    _tx: mpsc::Sender<String>,
}

impl RelayClient {
    /// Forward a JSON-RPC response to the relay.
    pub fn forward(&self, msg: String) {
        let _ = self._tx.try_send(msg);
    }
}

// ─── Spawn ────────────────────────────────────────────────────────────────────

/// Starts the relay background task if the license allows it.
/// Returns `None` if relay is disabled or token is absent.
pub async fn spawn_if_enabled(
    config: Arc<DaemonConfig>,
    license: &LicenseInfo,
    daemon_id: String,
    broadcaster: Arc<EventBroadcaster>,
) -> Option<Arc<RelayClient>> {
    if !license.features.relay {
        debug!("relay feature disabled — not connecting");
        return None;
    }

    let token = config.license_token.clone().unwrap_or_default();
    if token.is_empty() {
        warn!("relay feature enabled but no license token — skipping relay");
        return None;
    }

    let (tx, rx) = mpsc::channel::<String>(64);
    let client = Arc::new(RelayClient { _tx: tx });

    tokio::spawn(relay_loop(config, daemon_id, broadcaster, rx, token));

    Some(client)
}

// ─── Background loop ──────────────────────────────────────────────────────────

async fn relay_loop(
    config: Arc<DaemonConfig>,
    daemon_id: String,
    broadcaster: Arc<EventBroadcaster>,
    mut outbound_rx: mpsc::Receiver<String>,
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

                tokio::select! {
                    _ = handle_inbound(&mut stream, &broadcaster) => {
                        warn!("relay: inbound stream closed");
                    }
                    _ = handle_outbound(&mut outbound_rx, &mut sink) => {
                        warn!("relay: outbound sink closed");
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

async fn handle_inbound(
    stream: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    broadcaster: &Arc<EventBroadcaster>,
) {
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("relay: inbound frame ({} bytes)", text.len());
                broadcaster.broadcast("relay.frame", serde_json::json!({ "raw": text }));
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }
}

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

async fn sleep_backoff(backoff_secs: &mut u64) {
    info!("relay: reconnecting in {}s", *backoff_secs);
    tokio::time::sleep(std::time::Duration::from_secs(*backoff_secs)).await;
    *backoff_secs = (*backoff_secs * 2).min(60);
}
