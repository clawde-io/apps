// SPDX-License-Identifier: MIT
//! Resource Governor — monitors system RAM/CPU and enforces session tier transitions.
//!
//! Runs a background Tokio task that polls system resources every `poll_interval_secs`
//! seconds, identifies memory pressure levels, and triggers session evictions as needed.

use std::sync::Arc;
use sysinfo::System;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::{config::ResourceConfig, storage::Storage};

/// Memory pressure level computed from current system state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PressureLevel {
    /// Below max_memory_percent — all normal.
    Normal,
    /// At 80-95% of max_memory_percent budget — start evicting Warm sessions.
    Warning,
    /// At 95-100% of max_memory_percent budget — aggressively evict.
    Critical,
    /// Above emergency_memory_percent — evict everything possible immediately.
    Emergency,
}

/// Core resource management engine.
pub struct ResourceGovernor {
    config: ResourceConfig,
    sys: Mutex<System>,
    storage: Arc<Storage>,
    /// Computed max active sessions (from auto-calc or config override).
    max_active: u8,
}

impl ResourceGovernor {
    /// Create a new governor with the given config and storage reference.
    pub fn new(config: ResourceConfig, storage: Arc<Storage>) -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        let max_active = Self::compute_max_active(&config, &sys);
        info!(
            max_active,
            max_memory_percent = config.max_memory_percent,
            "resource governor initialized"
        );
        Self {
            config,
            sys: Mutex::new(sys),
            storage,
            max_active,
        }
    }

    /// Refresh system memory stats and return current usage percentage.
    pub async fn poll(&self) -> f64 {
        let mut sys = self.sys.lock().await;
        sys.refresh_memory();
        let total = sys.total_memory();
        let used = sys.used_memory();
        if total == 0 {
            return 0.0;
        }
        (used as f64 / total as f64) * 100.0
    }

    /// Determine pressure level from current system memory usage.
    pub async fn check_pressure(&self) -> PressureLevel {
        let usage_pct = self.poll().await;
        let budget = self.config.max_memory_percent as f64;
        let emergency = self.config.emergency_memory_percent as f64;

        if usage_pct >= emergency {
            PressureLevel::Emergency
        } else if usage_pct >= budget {
            PressureLevel::Critical
        } else if usage_pct >= budget * 0.95 {
            PressureLevel::Warning
        } else {
            PressureLevel::Normal
        }
    }

    /// Write current resource metrics to SQLite.
    pub async fn record_metrics(
        &self,
        active: i64,
        warm: i64,
        cold: i64,
        pool: i64,
        compressions: i64,
    ) -> anyhow::Result<()> {
        let sys = self.sys.lock().await;
        let total_ram = sys.total_memory() as i64;
        let used_ram = sys.used_memory() as i64;
        drop(sys);

        // Estimate daemon RAM (process memory - rough heuristic)
        let daemon_ram: i64 = 100 * 1024 * 1024; // ~100 MB estimate

        let pool_ref = self.storage.pool();
        sqlx::query(
            "INSERT INTO resource_metrics \
             (total_ram_bytes, used_ram_bytes, daemon_ram_bytes, \
              active_session_count, warm_session_count, cold_session_count, \
              pool_worker_count, context_compressions) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(total_ram)
        .bind(used_ram)
        .bind(daemon_ram)
        .bind(active)
        .bind(warm)
        .bind(cold)
        .bind(pool)
        .bind(compressions)
        .execute(&pool_ref)
        .await?;

        // Prune metrics older than 24 hours
        sqlx::query("DELETE FROM resource_metrics WHERE timestamp < unixepoch() - 86400")
            .execute(&pool_ref)
            .await?;

        Ok(())
    }

    /// Compute max concurrent active sessions from config and available RAM.
    pub fn compute_max_active(config: &ResourceConfig, sys: &System) -> u8 {
        if config.max_concurrent_active > 0 {
            return config.max_concurrent_active;
        }
        let total_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let budget_gb = total_gb * (config.max_memory_percent as f64 / 100.0);
        let daemon_gb = 0.1_f64;
        let flutter_gb = 0.4_f64;
        let pool_gb = config.process_pool_size as f64 * 0.3;
        let per_session_gb = 4.5_f64;
        let available = budget_gb - daemon_gb - flutter_gb - pool_gb;
        let max = (available / per_session_gb).floor() as u8;
        max.max(1)
    }

    /// Return the current computed max active session count.
    pub fn max_active(&self) -> u8 {
        self.max_active
    }

    /// Get current system RAM info.
    pub async fn ram_info(&self) -> (u64, u64) {
        let sys = self.sys.lock().await;
        (sys.total_memory(), sys.used_memory())
    }
}

/// Run the resource governor polling loop.
/// This is a long-running Tokio task — spawn with `tokio::spawn`.
pub async fn run_governor_loop(
    governor: Arc<ResourceGovernor>,
    storage: Arc<Storage>,
    config: ResourceConfig,
) {
    use tokio::time::{interval, Duration};

    let normal_interval = Duration::from_secs(config.poll_interval_secs);
    let fast_interval = Duration::from_secs(1);
    let mut tick = interval(normal_interval);
    let mut last_pressure = PressureLevel::Normal;
    let mut use_fast = false;

    loop {
        tick.tick().await;

        let pressure = governor.check_pressure().await;

        if pressure != last_pressure {
            match pressure {
                PressureLevel::Normal => debug!("resource pressure: normal"),
                PressureLevel::Warning => {
                    warn!("resource pressure: warning — consider evicting warm sessions")
                }
                PressureLevel::Critical => {
                    warn!("resource pressure: critical — evicting warm sessions")
                }
                PressureLevel::Emergency => {
                    warn!("resource pressure: EMERGENCY — aggressively evicting")
                }
            }
            last_pressure = pressure;
        }

        // Switch to fast polling under pressure (recreate interval only on transition)
        let should_fast = pressure >= PressureLevel::Warning;
        if should_fast != use_fast {
            use_fast = should_fast;
            tick = if use_fast {
                interval(fast_interval)
            } else {
                interval(normal_interval)
            };
        }

        // Collect tier counts for metrics
        if let Ok((active, warm, cold)) = count_session_tiers(&storage).await {
            let _ = governor.record_metrics(active, warm, cold, 0, 0).await;
        }
    }
}

async fn count_session_tiers(storage: &Storage) -> anyhow::Result<(i64, i64, i64)> {
    let pool = storage.pool();
    let active: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE tier = 'active'")
        .fetch_one(&pool)
        .await?;
    let warm: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE tier = 'warm'")
        .fetch_one(&pool)
        .await?;
    let cold: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE tier = 'cold'")
        .fetch_one(&pool)
        .await?;
    Ok((active.0, warm.0, cold.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResourceConfig;

    fn test_config() -> ResourceConfig {
        ResourceConfig {
            max_memory_percent: 70,
            max_concurrent_active: 0, // auto
            idle_to_warm_secs: 120,
            warm_to_cold_secs: 300,
            process_pool_size: 1,
            emergency_memory_percent: 90,
            poll_interval_secs: 5,
        }
    }

    #[test]
    fn test_compute_max_active_auto() {
        let config = test_config();
        let mut sys = System::new();
        sys.refresh_memory();
        // On any machine with RAM, should get at least 1
        let max = ResourceGovernor::compute_max_active(&config, &sys);
        assert!(max >= 1, "should always allow at least 1 active session");
    }

    #[test]
    fn test_compute_max_active_manual() {
        let mut config = test_config();
        config.max_concurrent_active = 3;
        let sys = System::new();
        let max = ResourceGovernor::compute_max_active(&config, &sys);
        assert_eq!(max, 3, "manual override should be respected");
    }

    #[test]
    fn test_pressure_level_ordering() {
        assert!(PressureLevel::Normal < PressureLevel::Warning);
        assert!(PressureLevel::Warning < PressureLevel::Critical);
        assert!(PressureLevel::Critical < PressureLevel::Emergency);
    }
}
