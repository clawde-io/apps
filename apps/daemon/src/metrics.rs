// SPDX-License-Identifier: MIT
// NOTE: This file is superseded by metrics/mod.rs (Sprint PP OB.1-3).
// Rust requires either metrics.rs OR metrics/mod.rs — not both.
// metrics/mod.rs is the canonical location. This file must be deleted
// when the build is cleaned up. Content has been migrated to metrics/mod.rs.
//!
//! Simple in-process counters exposed as `GET /metrics` in Prometheus text format.
//! No external library needed — all counters are `AtomicU64` incremented inline.
//!
//! Endpoint: `GET /metrics` on port 4300 (same port as the daemon WebSocket).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// In-process performance counters shared across all connections.
#[derive(Debug)]
pub struct DaemonMetrics {
    /// Total WebSocket sessions created since daemon start.
    pub sessions_created: AtomicU64,
    /// Total AI messages sent (user → daemon) since daemon start.
    pub messages_sent: AtomicU64,
    /// Total tool calls approved since daemon start.
    pub tool_calls_approved: AtomicU64,
    /// Total tool calls rejected (user reject + security block) since daemon start.
    pub tool_calls_rejected: AtomicU64,
    /// Total IPC rate limit hits (connections or RPC calls blocked) since daemon start.
    pub ipc_rate_limit_hits: AtomicU64,
    /// Total RPC requests dispatched since daemon start.
    pub rpc_requests_total: AtomicU64,
    /// Daemon start time — used to calculate uptime in the metrics response.
    pub started_at: Instant,
}

impl DaemonMetrics {
    pub fn new() -> Self {
        Self {
            sessions_created: AtomicU64::new(0),
            messages_sent: AtomicU64::new(0),
            tool_calls_approved: AtomicU64::new(0),
            tool_calls_rejected: AtomicU64::new(0),
            ipc_rate_limit_hits: AtomicU64::new(0),
            rpc_requests_total: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    pub fn inc_sessions_created(&self) {
        self.sessions_created.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_messages_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_tool_calls_approved(&self) {
        self.tool_calls_approved.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_tool_calls_rejected(&self) {
        self.tool_calls_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_ipc_rate_limit_hits(&self) {
        self.ipc_rate_limit_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_rpc_requests(&self) {
        self.rpc_requests_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Render counters in Prometheus text format.
    ///
    /// Active sessions count is passed in because it requires a lock (not stored here).
    pub fn render_prometheus(&self, active_sessions: u64) -> String {
        let uptime = self.started_at.elapsed().as_secs();
        let sessions_created = self.sessions_created.load(Ordering::Relaxed);
        let messages_sent = self.messages_sent.load(Ordering::Relaxed);
        let tool_calls_approved = self.tool_calls_approved.load(Ordering::Relaxed);
        let tool_calls_rejected = self.tool_calls_rejected.load(Ordering::Relaxed);
        let ipc_rate_limit_hits = self.ipc_rate_limit_hits.load(Ordering::Relaxed);
        let rpc_requests_total = self.rpc_requests_total.load(Ordering::Relaxed);

        format!(
            "# HELP clawd_uptime_seconds Daemon uptime in seconds.\n\
             # TYPE clawd_uptime_seconds gauge\n\
             clawd_uptime_seconds {uptime}\n\
             # HELP clawd_active_sessions Current number of active sessions.\n\
             # TYPE clawd_active_sessions gauge\n\
             clawd_active_sessions {active_sessions}\n\
             # HELP clawd_sessions_created_total Total sessions created since daemon start.\n\
             # TYPE clawd_sessions_created_total counter\n\
             clawd_sessions_created_total {sessions_created}\n\
             # HELP clawd_messages_sent_total Total AI messages sent since daemon start.\n\
             # TYPE clawd_messages_sent_total counter\n\
             clawd_messages_sent_total {messages_sent}\n\
             # HELP clawd_tool_calls_approved_total Total tool calls approved since daemon start.\n\
             # TYPE clawd_tool_calls_approved_total counter\n\
             clawd_tool_calls_approved_total {tool_calls_approved}\n\
             # HELP clawd_tool_calls_rejected_total Total tool calls rejected since daemon start.\n\
             # TYPE clawd_tool_calls_rejected_total counter\n\
             clawd_tool_calls_rejected_total {tool_calls_rejected}\n\
             # HELP clawd_ipc_rate_limit_hits_total IPC rate limit hits since daemon start.\n\
             # TYPE clawd_ipc_rate_limit_hits_total counter\n\
             clawd_ipc_rate_limit_hits_total {ipc_rate_limit_hits}\n\
             # HELP clawd_rpc_requests_total Total RPC requests dispatched since daemon start.\n\
             # TYPE clawd_rpc_requests_total counter\n\
             clawd_rpc_requests_total {rpc_requests_total}\n"
        )
    }
}

impl Default for DaemonMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared handle — cheaply clonable.
pub type SharedMetrics = Arc<DaemonMetrics>;
