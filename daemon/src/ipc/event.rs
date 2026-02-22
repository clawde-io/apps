use serde_json::Value;
use tokio::sync::broadcast;

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
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    /// Send a JSON-RPC notification to all connected clients.
    pub fn broadcast(&self, method: &str, params: Value) {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        // Ignore errors — no subscribers is fine
        let _ = self
            .tx
            .send(serde_json::to_string(&notification).unwrap_or_default());
    }

    /// Subscribe to all broadcast events.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}
