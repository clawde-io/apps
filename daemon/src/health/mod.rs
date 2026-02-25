// SPDX-License-Identifier: MIT
//! Daemon health check system.
//!
//! Provides [`HealthReporter`] that aggregates multiple [`SystemHealthCheck`]
//! implementations into a single [`HealthReport`].
//!
//! # Included checks
//! - [`DatabaseHealthCheck`] — verifies SQLite can handle a `SELECT 1`
//! - [`StorageHealthCheck`] — verifies the data directory is writable
//! - [`ProviderHealthCheck`] — verifies at least one provider CLI is on PATH
//!
//! # Usage
//! ```rust,no_run
//! use clawd::health::{
//!     reporter::HealthReporter,
//!     checks::{DatabaseHealthCheck, StorageHealthCheck, ProviderHealthCheck},
//! };
//!
//! let reporter = HealthReporter::new()
//!     .with_check(DatabaseHealthCheck::new(pool))
//!     .with_check(StorageHealthCheck::new(&data_dir))
//!     .with_check(ProviderHealthCheck::new());
//!
//! let report = reporter.get_health_report().await;
//! println!("overall: {}", report.status);
//! ```

pub mod checks;
pub mod reporter;

// Convenience re-exports.
pub use checks::{
    CheckResult, CheckStatus, DatabaseHealthCheck, ProviderHealthCheck, StorageHealthCheck,
    SystemHealthCheck,
};
pub use reporter::{HealthReport, HealthReporter};
