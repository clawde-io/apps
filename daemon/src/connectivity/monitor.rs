//! Connection quality monitor.
//!
//! Pings the relay (or direct/VPN peer) every 30 seconds, records round-trip
//! time and estimates packet loss. The result is exposed via the
//! `connectivity.status` JSON-RPC method and fires a `connectivity_degraded`
//! push event when quality drops below thresholds.

use crate::ipc::event::EventBroadcaster;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

const PING_INTERVAL_SECS: u64 = 30;
const DEGRADED_RTT_MS: u64 = 500;
const DEGRADED_LOSS_PCT: f32 = 5.0;
const PING_TIMEOUT_SECS: u64 = 5;

/// Current connection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMode {
    Relay,
    Direct,
    Vpn,
    Offline,
}

impl std::fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Relay => write!(f, "relay"),
            Self::Direct => write!(f, "direct"),
            Self::Vpn => write!(f, "vpn"),
            Self::Offline => write!(f, "offline"),
        }
    }
}

/// Snapshot of connection quality.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QualitySnapshot {
    pub mode: ConnectionMode,
    /// Round-trip time to the relay/peer in milliseconds. 0 if not yet measured.
    pub rtt_ms: u64,
    /// Estimated packet loss percentage over the last window. 0.0 if not yet measured.
    pub packet_loss_pct: f32,
    /// Unix timestamp of the last successful ping.
    pub last_ping_at: i64,
    /// `true` when quality is below degradation thresholds.
    pub degraded: bool,
}

impl Default for QualitySnapshot {
    fn default() -> Self {
        Self {
            mode: ConnectionMode::Relay,
            rtt_ms: 0,
            packet_loss_pct: 0.0,
            last_ping_at: 0,
            degraded: false,
        }
    }
}

/// Shared quality state updated by the background monitor task.
pub type SharedQuality = Arc<RwLock<QualitySnapshot>>;

pub fn new_shared_quality() -> SharedQuality {
    Arc::new(RwLock::new(QualitySnapshot::default()))
}

/// Background task that pings `target_url` every 30 seconds and updates `quality`.
///
/// Fires `connectivity_degraded` event on the broadcaster when quality drops.
/// Runs until the Tokio runtime shuts down.
pub async fn run_monitor(
    target_url: String,
    quality: SharedQuality,
    broadcaster: Arc<EventBroadcaster>,
    mode: ConnectionMode,
) {
    info!(target = %target_url, mode = %mode, "connectivity monitor started");
    let mut interval = tokio::time::interval(Duration::from_secs(PING_INTERVAL_SECS));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(PING_TIMEOUT_SECS))
        .build()
        .unwrap_or_default();

    // Rolling window: track last 10 ping outcomes (true = success, false = loss)
    let mut window: std::collections::VecDeque<bool> =
        std::collections::VecDeque::with_capacity(10);
    let mut was_degraded = false;

    loop {
        interval.tick().await;

        let ping_start = Instant::now();
        // Use an HTTP HEAD request to the target URL as a lightweight ping.
        // For WebSocket relay URLs we probe the HTTP equivalent.
        let probe_url = target_url
            .replace("wss://", "https://")
            .replace("ws://", "http://")
            .trim_end_matches("/ws")
            .to_string()
            + "/health";

        let success = client.head(&probe_url).send().await.is_ok();
        let rtt_ms = ping_start.elapsed().as_millis() as u64;

        // Update rolling window
        if window.len() >= 10 {
            window.pop_front();
        }
        window.push_back(success);

        let loss_count = window.iter().filter(|&&ok| !ok).count();
        let packet_loss_pct = if window.is_empty() {
            0.0
        } else {
            (loss_count as f32 / window.len() as f32) * 100.0
        };

        let measured_rtt = if success { rtt_ms } else { u64::MAX };
        let degraded = measured_rtt > DEGRADED_RTT_MS || packet_loss_pct > DEGRADED_LOSS_PCT;

        debug!(
            rtt_ms = measured_rtt,
            loss_pct = packet_loss_pct,
            degraded = degraded,
            "connectivity ping"
        );

        // Update shared state
        {
            let mut snap = quality.write().await;
            snap.mode = mode;
            snap.rtt_ms = if success { measured_rtt } else { 0 };
            snap.packet_loss_pct = packet_loss_pct;
            snap.last_ping_at = chrono::Utc::now().timestamp();
            snap.degraded = degraded;
        }

        // Fire event if degradation state changed
        if degraded && !was_degraded {
            warn!(
                rtt_ms = measured_rtt,
                loss_pct = packet_loss_pct,
                "connectivity degraded"
            );
            broadcaster.broadcast(
                "connectivity_degraded",
                json!({
                    "mode": mode.to_string(),
                    "rtt_ms": measured_rtt,
                    "packet_loss_pct": packet_loss_pct,
                }),
            );
        } else if !degraded && was_degraded {
            info!(rtt_ms = measured_rtt, "connectivity restored");
            broadcaster.broadcast(
                "connectivity_restored",
                json!({
                    "mode": mode.to_string(),
                    "rtt_ms": measured_rtt,
                }),
            );
        }

        was_degraded = degraded;
    }
}
