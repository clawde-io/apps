// SPDX-License-Identifier: MIT
//! Health reporter — aggregates all [`SystemHealthCheck`] results into a [`HealthReport`].
//!
//! The reporter runs all registered checks concurrently and derives an overall
//! status from the worst individual result:
//! - All checks `ok` → overall `ok`
//! - Any check `degraded`, none `critical` → overall `degraded`
//! - Any check `critical` → overall `critical`

use crate::health::checks::{CheckResult, CheckStatus, SystemHealthCheck};
use chrono::Utc;
use std::sync::Arc;
use tracing::debug;

/// Aggregated health report returned by [`HealthReporter::get_health_report`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthReport {
    /// Overall status: `"ok"`, `"degraded"`, or `"critical"`.
    pub status: String,
    /// Individual check results.
    pub checks: Vec<CheckResult>,
    /// ISO-8601 timestamp when this report was generated.
    pub timestamp: String,
    /// Daemon version string.
    pub version: String,
}

impl HealthReport {
    /// Returns `true` if the overall status is `"ok"`.
    pub fn is_healthy(&self) -> bool {
        self.status == "ok"
    }
}

/// Runs all registered health checks and aggregates the results.
///
/// # Example
/// ```rust,no_run
/// use clawd::health::{
///     reporter::HealthReporter,
///     checks::{DatabaseHealthCheck, StorageHealthCheck, ProviderHealthCheck},
/// };
///
/// let reporter = HealthReporter::new()
///     .with_check(DatabaseHealthCheck::new(pool))
///     .with_check(StorageHealthCheck::new(&data_dir))
///     .with_check(ProviderHealthCheck::new());
///
/// let report = reporter.get_health_report().await;
/// ```
pub struct HealthReporter {
    checks: Vec<Arc<dyn SystemHealthCheck>>,
}

impl HealthReporter {
    /// Create a new reporter with no checks registered.
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
        }
    }

    /// Register a health check.
    ///
    /// Checks are run concurrently when [`get_health_report`](Self::get_health_report) is called.
    pub fn with_check(mut self, check: impl SystemHealthCheck + 'static) -> Self {
        self.checks.push(Arc::new(check));
        self
    }

    /// Register a boxed health check (useful when the concrete type is erased).
    pub fn with_boxed_check(mut self, check: Arc<dyn SystemHealthCheck>) -> Self {
        self.checks.push(check);
        self
    }

    /// Run all registered checks concurrently and return the aggregated [`HealthReport`].
    ///
    /// Each check runs in its own `tokio::spawn` task so that a hung check cannot
    /// block the others. Checks that panic or are dropped are reported as `critical`.
    pub async fn get_health_report(&self) -> HealthReport {
        debug!("running {} health checks", self.checks.len());

        // Spawn all checks concurrently.
        let handles: Vec<_> = self
            .checks
            .iter()
            .map(|check| {
                let check = Arc::clone(check);
                tokio::spawn(async move { check.run().await })
            })
            .collect();

        // Collect results, treating JoinErrors as critical.
        let mut results: Vec<CheckResult> = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(CheckResult {
                        name: "unknown".to_string(),
                        message: format!("health check panicked: {e}"),
                        status: CheckStatus::Critical,
                        checked_at: Utc::now().to_rfc3339(),
                        latency_ms: None,
                    });
                }
            }
        }

        // Derive overall status from worst individual result.
        let overall = results
            .iter()
            .fold(CheckStatus::Ok, |acc, r| CheckStatus::worst(acc, r.status.clone()));

        HealthReport {
            status: overall.to_string(),
            checks: results,
            timestamp: Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl Default for HealthReporter {
    fn default() -> Self {
        Self::new()
    }
}
