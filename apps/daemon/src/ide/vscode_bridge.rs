// SPDX-License-Identifier: MIT
//! VS Code extension bridge — Sprint Z, IE.T02–IE.T04.
//!
//! Connects `clawd` to the ClawDE VS Code extension.  The extension
//! communicates over the same JSON-RPC 2.0 WebSocket as every other client,
//! but registers itself via `ide.extensionConnected` so the daemon knows it
//! is an IDE rather than a Flutter app.
//!
//! This module holds the in-memory state of connected VS Code extensions and
//! the most-recent [`EditorContext`] reported by each one.

use crate::ide::editor_context::{EditorContext, IdeConnectionRecord};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

// ─── VsCodeBridge ─────────────────────────────────────────────────────────────

/// Shared, in-memory state for all connected IDE extensions.
///
/// Wrapped in `Arc<RwLock<…>>` so every handler clone can access the same
/// registry without copying.
#[derive(Debug, Default)]
pub struct VsCodeBridge {
    /// Active IDE connections keyed by `connection_id`.
    connections: HashMap<String, IdeConnectionRecord>,
    /// Most-recent editor context per `connection_id`.
    contexts: HashMap<String, EditorContext>,
}

impl VsCodeBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new IDE extension connection and return the assigned `connection_id`.
    pub fn register_connection(
        &mut self,
        extension_type: &str,
        extension_version: Option<&str>,
    ) -> String {
        let connection_id = Uuid::new_v4().to_string();
        let now = crate::ide::now_utc();
        let record = IdeConnectionRecord {
            connection_id: connection_id.clone(),
            extension_type: extension_type.to_string(),
            extension_version: extension_version.map(str::to_string),
            connected_at: now.clone(),
            last_seen_at: now,
        };
        self.connections.insert(connection_id.clone(), record);
        info!(
            connection_id = %connection_id,
            extension_type = %extension_type,
            "IDE extension connected"
        );
        connection_id
    }

    /// Remove a connection on clean disconnect or timeout.
    pub fn remove_connection(&mut self, connection_id: &str) {
        self.connections.remove(connection_id);
        self.contexts.remove(connection_id);
        debug!(connection_id = %connection_id, "IDE extension disconnected");
    }

    /// Update the editor context for a connection and refresh `last_seen_at`.
    pub fn update_context(&mut self, connection_id: &str, ctx: EditorContext) {
        if let Some(conn) = self.connections.get_mut(connection_id) {
            conn.last_seen_at = crate::ide::now_utc();
        }
        self.contexts.insert(connection_id.to_string(), ctx);
    }

    /// Return the most-recent editor context for a given connection, if any.
    pub fn get_context(&self, connection_id: &str) -> Option<&EditorContext> {
        self.contexts.get(connection_id)
    }

    /// Return the most-recent editor context from any connected VS Code extension.
    ///
    /// When multiple extensions are connected, the one with the latest
    /// `updated_at` timestamp wins.
    pub fn latest_context(&self) -> Option<&EditorContext> {
        self.contexts
            .values()
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
    }

    /// Return a snapshot of all currently-connected IDE extensions.
    pub fn list_connections(&self) -> Vec<&IdeConnectionRecord> {
        self.connections.values().collect()
    }

    /// Return the count of currently-connected extensions.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }
}

// ─── SharedVsCodeBridge ───────────────────────────────────────────────────────

/// Thread-safe shared handle to the VS Code bridge state.
pub type SharedVsCodeBridge = Arc<RwLock<VsCodeBridge>>;

/// Construct the shared bridge used by the daemon.
pub fn new_shared_bridge() -> SharedVsCodeBridge {
    Arc::new(RwLock::new(VsCodeBridge::new()))
}
