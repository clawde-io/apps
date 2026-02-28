// SPDX-License-Identifier: MIT
//! Observability utilities — Phase 47.
//!
//! Structured logging helpers, RPC latency tracking, and health check endpoint.

use std::time::Instant;
use tracing::{debug, info};

/// Track latency of an async operation and emit a structured log event.
pub struct LatencyTracker {
    operation: String,
    start: Instant,
}

impl LatencyTracker {
    /// Start tracking latency for an operation.
    ///
    /// Examples:
    ///   let tracker = LatencyTracker::start("session.create");
    pub fn start(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            start: Instant::now(),
        }
    }

    /// Finish tracking and emit a log event with the elapsed time.
    pub fn finish(self) {
        let elapsed_ms = self.start.elapsed().as_millis();
        if elapsed_ms > 1000 {
            // Slow operation — log at info level
            info!(
                operation = %self.operation,
                elapsed_ms = elapsed_ms,
                "slow operation"
            );
        } else {
            debug!(
                operation = %self.operation,
                elapsed_ms = elapsed_ms,
                "operation complete"
            );
        }
    }
}

/// Health check status.
#[derive(Debug, serde::Serialize)]
pub struct HealthStatus {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_secs: u64,
    pub db_ok: bool,
}

impl HealthStatus {
    pub fn ok(uptime_secs: u64, db_ok: bool) -> Self {
        Self {
            status: if db_ok { "ok" } else { "degraded" },
            version: env!("CARGO_PKG_VERSION"),
            uptime_secs,
            db_ok,
        }
    }
}

/// Format bytes into a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
    }

    #[test]
    fn test_health_status_ok() {
        let h = HealthStatus::ok(300, true);
        assert_eq!(h.status, "ok");
    }

    #[test]
    fn test_health_status_degraded() {
        let h = HealthStatus::ok(300, false);
        assert_eq!(h.status, "degraded");
    }
}
