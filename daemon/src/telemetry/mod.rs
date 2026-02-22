//! Privacy-safe telemetry — no code, no file paths, no user content.
//!
//! Events are queued in memory and flushed to POST /telemetry every 60 seconds
//! or when 20 events accumulate, whichever comes first.
//! Flush failures are logged and silently dropped — telemetry never blocks the daemon.

use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::config::DaemonConfig;

const FLUSH_INTERVAL_SECS: u64 = 60;
const FLUSH_BATCH_SIZE: usize = 20;

// ─── Event types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEvent {
    pub event: String,
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

impl TelemetryEvent {
    pub fn new(event: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            ts: Utc::now().to_rfc3339(),
            provider: None,
        }
    }

    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }
}

// ─── Sender handle ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TelemetrySender {
    tx: mpsc::Sender<TelemetryEvent>,
}

impl TelemetrySender {
    /// Queue an event for the next flush.  Never blocks — drops silently if queue full.
    pub fn send(&self, event: TelemetryEvent) {
        let _ = self.tx.try_send(event);
    }
}

// ─── Background flush task ────────────────────────────────────────────────────

/// Spawns the background telemetry flush task and returns a `TelemetrySender`.
///
/// The task flushes on a 60s timer or when 20 events accumulate.
/// If `daemon_id` is empty, events are accepted but discarded on flush
/// (no network call when identity is unknown).
pub fn spawn(config: Arc<DaemonConfig>, daemon_id: String, tier: String) -> TelemetrySender {
    let (tx, mut rx) = mpsc::channel::<TelemetryEvent>(200);
    let platform = std::env::consts::OS.to_string();
    let version = env!("CARGO_PKG_VERSION").to_string();

    tokio::spawn(async move {
        let mut buffer: Vec<TelemetryEvent> = Vec::new();
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(FLUSH_INTERVAL_SECS));
        interval.tick().await; // skip immediate tick

        loop {
            tokio::select! {
                // Accumulate incoming events
                Some(event) = rx.recv() => {
                    buffer.push(event);
                    if buffer.len() >= FLUSH_BATCH_SIZE {
                        flush(&config, &daemon_id, &tier, &platform, &version, &mut buffer).await;
                    }
                }
                // Periodic flush
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        flush(&config, &daemon_id, &tier, &platform, &version, &mut buffer).await;
                    }
                }
                // Channel closed (daemon shutting down)
                else => break,
            }
        }

        // Final flush on shutdown
        if !buffer.is_empty() {
            flush(&config, &daemon_id, &tier, &platform, &version, &mut buffer).await;
        }
    });

    TelemetrySender { tx }
}

async fn flush(
    config: &DaemonConfig,
    daemon_id: &str,
    tier: &str,
    platform: &str,
    version: &str,
    buffer: &mut Vec<TelemetryEvent>,
) {
    if daemon_id.is_empty() {
        debug!(
            "telemetry: daemon_id not set, discarding {} events",
            buffer.len()
        );
        buffer.clear();
        return;
    }

    let events = std::mem::take(buffer);
    let count = events.len();

    let payload = serde_json::json!({
        "daemonId": daemon_id,
        "tier": tier,
        "platform": platform,
        "daemonVersion": version,
        "events": events,
    });

    let url = format!("{}/telemetry", config.api_base_url);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("telemetry: failed to build HTTP client: {e:#}");
            return;
        }
    };

    match client.post(&url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => {
            debug!("telemetry: flushed {count} events");
        }
        Ok(resp) => {
            warn!("telemetry: server returned {}", resp.status());
        }
        Err(e) => {
            warn!("telemetry: flush failed: {e:#}");
        }
    }
}
