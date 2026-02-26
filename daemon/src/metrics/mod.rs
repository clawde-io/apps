// metrics/mod.rs — Daemon metrics: Prometheus counters (DC.T49) + session cost
//                  tracking + budget enforcement (Sprint PP OB.1-3).
//
// This module replaces the former `metrics.rs` file. All existing imports of
// `metrics::SharedMetrics`, `metrics::DaemonMetrics` continue to work.

pub mod budget;
pub mod cost;
pub mod store;

pub use budget::{evaluate_budget, BudgetStatus};
pub use cost::calculate_cost;
pub use store::{MetricEntry, MetricRollup, MetricsStore, MetricsSummary};

// ── Prometheus in-process counters (originally metrics.rs / DC.T49) ──────────

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// In-process performance counters shared across all connections.
#[derive(Debug)]
pub struct DaemonMetrics {
    pub sessions_created: AtomicU64,
    pub messages_sent: AtomicU64,
    pub tool_calls_approved: AtomicU64,
    pub tool_calls_rejected: AtomicU64,
    pub ipc_rate_limit_hits: AtomicU64,
    pub rpc_requests_total: AtomicU64,
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

    pub fn render_prometheus(&self, active_sessions: u64) -> String {
        let uptime = self.started_at.elapsed().as_secs();
        let sc = self.sessions_created.load(Ordering::Relaxed);
        let ms = self.messages_sent.load(Ordering::Relaxed);
        let tca = self.tool_calls_approved.load(Ordering::Relaxed);
        let tcr = self.tool_calls_rejected.load(Ordering::Relaxed);
        let rl = self.ipc_rate_limit_hits.load(Ordering::Relaxed);
        let rpc = self.rpc_requests_total.load(Ordering::Relaxed);
        format!(
            "# TYPE clawd_uptime_seconds gauge\nclawd_uptime_seconds {uptime}\n\
             # TYPE clawd_active_sessions gauge\nclawd_active_sessions {active_sessions}\n\
             # TYPE clawd_sessions_created_total counter\nclawd_sessions_created_total {sc}\n\
             # TYPE clawd_messages_sent_total counter\nclawd_messages_sent_total {ms}\n\
             # TYPE clawd_tool_calls_approved_total counter\nclawd_tool_calls_approved_total {tca}\n\
             # TYPE clawd_tool_calls_rejected_total counter\nclawd_tool_calls_rejected_total {tcr}\n\
             # TYPE clawd_ipc_rate_limit_hits_total counter\nclawd_ipc_rate_limit_hits_total {rl}\n\
             # TYPE clawd_rpc_requests_total counter\nclawd_rpc_requests_total {rpc}\n"
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
