use serde_json::Value;
use tokio::sync::broadcast;
use tracing::warn;

/// Broadcasts JSON-RPC notification strings to all connected WebSocket clients.
#[derive(Clone)]
pub struct EventBroadcaster {
    tx: broadcast::Sender<String>,
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBroadcaster {
    pub fn new() -> Self {
        // 4096-message channel gives headroom for bursts of tool-call events
        // across many concurrent sessions before lagging receivers are dropped.
        let (tx, _) = broadcast::channel(4096);
        Self { tx }
    }

    /// Send a JSON-RPC notification to all connected clients.
    pub fn broadcast(&self, method: &str, params: Value) {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        match serde_json::to_string(&notification) {
            Ok(json) => {
                // Ignore send errors — no subscribers is fine
                let _ = self.tx.send(json);
            }
            Err(e) => {
                warn!(method = method, err = %e, "failed to serialize broadcast event");
            }
        }
    }

    /// Subscribe to all broadcast events.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}
