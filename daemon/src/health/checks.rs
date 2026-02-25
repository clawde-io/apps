// SPDX-License-Identifier: MIT
//! Individual health check implementations.
//!
//! Each check implements the [`SystemHealthCheck`] trait and reports whether a
//! specific subsystem is healthy, degraded, or unavailable.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

/// Severity level reported by a health check.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// The subsystem is operating normally.
    Ok,
    /// The subsystem is functional but degraded (e.g., high latency, non-critical failure).
    Degraded,
    /// The subsystem is unavailable or critically broken.
    Critical,
}

impl CheckStatus {
    /// Returns the worst (highest-severity) of two statuses.
    pub fn worst(a: CheckStatus, b: CheckStatus) -> CheckStatus {
        match (&a, &b) {
            (CheckStatus::Critical, _) | (_, CheckStatus::Critical) => CheckStatus::Critical,
            (CheckStatus::Degraded, _) | (_, CheckStatus::Degraded) => CheckStatus::Degraded,
            _ => CheckStatus::Ok,
        }
    }
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckStatus::Ok => write!(f, "ok"),
            CheckStatus::Degraded => write!(f, "degraded"),
            CheckStatus::Critical => write!(f, "critical"),
        }
    }
}

/// Result of running a single health check.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckResult {
    /// Machine-readable name of this check (e.g., `"database"`, `"storage"`).
    pub name: String,
    /// Human-readable message describing the result.
    pub message: String,
    /// Status of this check.
    pub status: CheckStatus,
    /// ISO-8601 timestamp when the check was run.
    pub checked_at: String,
    /// Optional latency measurement (e.g., database query round-trip in ms).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl CheckResult {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
            status: CheckStatus::Ok,
            checked_at: Utc::now().to_rfc3339(),
            latency_ms: None,
        }
    }

    fn ok_with_latency(
        name: impl Into<String>,
        message: impl Into<String>,
        latency_ms: u64,
    ) -> Self {
        Self {
            latency_ms: Some(latency_ms),
            ..Self::ok(name, message)
        }
    }

    fn degraded(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
            status: CheckStatus::Degraded,
            checked_at: Utc::now().to_rfc3339(),
            latency_ms: None,
        }
    }

    fn critical(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
            status: CheckStatus::Critical,
            checked_at: Utc::now().to_rfc3339(),
            latency_ms: None,
        }
    }
}

/// Async health check trait.
///
/// Implement this for any subsystem that needs to be checked during
/// `health.getReport` or on the `/health` endpoint.
#[async_trait]
pub trait SystemHealthCheck: Send + Sync {
    /// Run the check and return a result.
    async fn run(&self) -> CheckResult;
}

// ─── Database check ───────────────────────────────────────────────────────────

/// Checks that the SQLite database pool can execute a simple query.
pub struct DatabaseHealthCheck {
    pool: SqlitePool,
}

impl DatabaseHealthCheck {
    /// Create a new check using the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SystemHealthCheck for DatabaseHealthCheck {
    async fn run(&self) -> CheckResult {
        let start = std::time::Instant::now();
        let result: Result<(i64,), sqlx::Error> =
            sqlx::query_as("SELECT 1").fetch_one(&self.pool).await;
        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(_) => CheckResult::ok_with_latency(
                "database",
                format!("SQLite reachable ({latency_ms}ms)"),
                latency_ms,
            ),
            Err(e) => CheckResult::critical("database", format!("SQLite query failed: {e}")),
        }
    }
}

// ─── Storage check ────────────────────────────────────────────────────────────

/// Checks that the data directory exists and is writable.
pub struct StorageHealthCheck {
    data_dir: PathBuf,
}

impl StorageHealthCheck {
    /// Create a new check for the given data directory.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    fn is_writable(path: &Path) -> bool {
        // Attempt to create a temporary file in the directory.
        let probe = path.join(".health_probe");
        match std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&probe)
        {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe);
                true
            }
            Err(_) => false,
        }
    }
}

#[async_trait]
impl SystemHealthCheck for StorageHealthCheck {
    async fn run(&self) -> CheckResult {
        let path = self.data_dir.clone();
        // Run the blocking FS operations on a thread pool thread.
        let result = tokio::task::spawn_blocking(move || {
            if !path.exists() {
                return Err(format!("data_dir does not exist: {}", path.display()));
            }
            if !path.is_dir() {
                return Err(format!("data_dir is not a directory: {}", path.display()));
            }
            if !Self::is_writable(&path) {
                return Err(format!("data_dir is not writable: {}", path.display()));
            }
            Ok(path)
        })
        .await;

        match result {
            Ok(Ok(p)) => CheckResult::ok(
                "storage",
                format!("data_dir writable: {}", p.display()),
            ),
            Ok(Err(msg)) => CheckResult::critical("storage", msg),
            Err(e) => CheckResult::critical("storage", format!("spawn_blocking error: {e}")),
        }
    }
}

// ─── Provider check ───────────────────────────────────────────────────────────

/// Provider CLI binary names to probe for.
const PROVIDER_BINARIES: &[(&str, &str)] = &[
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("cursor", "Cursor"),
];

/// Checks that at least one supported provider CLI is available on PATH.
pub struct ProviderHealthCheck;

impl ProviderHealthCheck {
    /// Create a new provider availability check.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProviderHealthCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SystemHealthCheck for ProviderHealthCheck {
    async fn run(&self) -> CheckResult {
        let found: Vec<String> = tokio::task::spawn_blocking(|| {
            PROVIDER_BINARIES
                .iter()
                .filter_map(|(bin, label)| {
                    which_bin(bin).map(|_| (*label).to_string())
                })
                .collect()
        })
        .await
        .unwrap_or_default();

        if found.is_empty() {
            CheckResult::degraded(
                "provider",
                format!(
                    "No provider CLI found on PATH. Checked: {}",
                    PROVIDER_BINARIES
                        .iter()
                        .map(|(b, _)| *b)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        } else {
            CheckResult::ok("provider", format!("Available providers: {}", found.join(", ")))
        }
    }
}

/// Minimal `which`-equivalent: returns `Some(path)` if the binary is on PATH.
fn which_bin(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path_var| {
        std::env::split_paths(&path_var).find_map(|dir| {
            let candidate = dir.join(name);
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}
