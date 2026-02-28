//! Vendor session snapshots for conversation threading (Phase 43f).
//!
//! When a task thread hands off to a new turn, we snapshot the vendor-specific
//! session ID (Claude's `sessionId`, Codex's conversation ID, etc.) so that
//! the next turn can resume the conversation without replaying history.
//!
//! The snapshot store is an in-memory `HashMap` keyed by `(thread_id, vendor)`,
//! backed by a `tokio::sync::RwLock` for concurrent access.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

/// A point-in-time snapshot of a vendor AI session attached to a thread.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub thread_id: String,
    /// `"claude"` | `"codex"` — identifies the vendor that issued the session ID.
    pub vendor: String,
    /// Opaque vendor-specific session identifier (e.g. Claude session UUID).
    pub vendor_session_id: String,
    /// Model configuration active at the time of the snapshot.
    pub model_config: serde_json::Value,
    pub snapshot_at: DateTime<Utc>,
}

/// Composite key for the snapshot map.
type SnapshotKey = (String, String); // (thread_id, vendor)

/// In-memory store for the latest vendor session snapshot per thread.
///
/// Only the **most recent** snapshot per `(thread_id, vendor)` pair is kept
/// in memory. Historical snapshots are not needed for session resume — we
/// always want the latest one.
#[derive(Clone)]
pub struct SessionSnapshotStore {
    inner: Arc<RwLock<HashMap<SnapshotKey, SessionSnapshot>>>,
}

impl SessionSnapshotStore {
    /// Create a new, empty store.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Save (or replace) the latest snapshot for `(thread_id, vendor)`.
    pub async fn save_snapshot(
        &self,
        thread_id: impl Into<String>,
        vendor: impl Into<String>,
        vendor_session_id: impl Into<String>,
        model_config: serde_json::Value,
    ) -> Result<()> {
        let thread_id = thread_id.into();
        let vendor = vendor.into();
        let key = (thread_id.clone(), vendor.clone());
        let snapshot = SessionSnapshot {
            thread_id,
            vendor,
            vendor_session_id: vendor_session_id.into(),
            model_config,
            snapshot_at: Utc::now(),
        };
        let mut map = self.inner.write().await;
        map.insert(key, snapshot);
        Ok(())
    }

    /// Retrieve the latest snapshot for a `(thread_id, vendor)` pair.
    ///
    /// Returns `None` if no snapshot has been saved yet.
    pub async fn get_latest(&self, thread_id: &str, vendor: &str) -> Option<SessionSnapshot> {
        let map = self.inner.read().await;
        map.get(&(thread_id.to_string(), vendor.to_string()))
            .cloned()
    }

    /// Remove all snapshots for a thread (e.g. when the thread is archived).
    pub async fn clear_thread(&self, thread_id: &str) {
        let mut map = self.inner.write().await;
        map.retain(|(tid, _), _| tid != thread_id);
    }
}

impl Default for SessionSnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}
