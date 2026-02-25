// SPDX-License-Identifier: MIT
//! Circuit breaker pattern for external service calls.
//!
//! Protects provider API calls, relay connections, and the license endpoint from
//! cascading failures. When a service starts failing repeatedly, the circuit
//! opens and requests fail fast instead of blocking threads waiting for timeouts.
//!
//! # State machine
//!
//! ```text
//! Closed ──(failure_threshold failures)──► Open
//!   ▲                                        │
//!   └──(success_threshold successes)──── HalfOpen ◄─(timeout elapsed)──┘
//! ```
//!
//! - **Closed**: All calls are allowed. Failures are counted.
//! - **Open**: All calls are rejected immediately (fast-fail). After `timeout` elapses,
//!   the breaker transitions to HalfOpen to test recovery.
//! - **HalfOpen**: A limited probe call is allowed through. If it succeeds, count successes;
//!   once `success_threshold` successes are recorded, close the circuit. If it fails,
//!   return to Open and reset the timeout.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Observable state of a circuit breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — calls are allowed.
    Closed,
    /// Failing — calls are rejected immediately without attempting the operation.
    Open,
    /// Testing recovery — one probe call is allowed through.
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half_open"),
        }
    }
}

/// Configuration for a [`CircuitBreaker`].
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before the circuit opens.
    ///
    /// Default: 5
    pub failure_threshold: u32,
    /// Number of consecutive successes (from HalfOpen) before the circuit closes.
    ///
    /// Default: 2
    pub success_threshold: u32,
    /// How long the circuit stays Open before transitioning to HalfOpen for a probe.
    ///
    /// Default: 30 seconds
    pub timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Internal mutable state guarded by an `RwLock`.
#[derive(Debug)]
struct BreakerInner {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure: Option<Instant>,
}

impl BreakerInner {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: None,
        }
    }
}

/// Thread-safe circuit breaker.
///
/// Cheaply cloneable — all clones share the same internal state via `Arc`.
///
/// # Example
/// ```rust,ignore
/// use clawd::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
///
/// let cb = CircuitBreaker::new("claude-api", CircuitBreakerConfig::default());
/// // Before making an external call:
/// if cb.is_allowed().await {
///     match call_external_service().await {
///         Ok(r)  => { cb.record_success().await; /* use r */ }
///         Err(e) => { cb.record_failure().await; /* handle e */ }
///     }
/// } else {
///     // fast-fail — circuit is open
/// }
/// ```
#[derive(Clone)]
pub struct CircuitBreaker {
    inner: Arc<RwLock<BreakerInner>>,
    config: Arc<CircuitBreakerConfig>,
    name: Arc<str>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given name and configuration.
    ///
    /// The breaker starts in the `Closed` state.
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BreakerInner::new())),
            config: Arc::new(config),
            name: Arc::from(name.into().as_str()),
        }
    }

    /// Returns `true` if a call should be attempted.
    ///
    /// - `Closed` → always `true`
    /// - `Open`   → `false`, unless the timeout has elapsed, in which case the
    ///   breaker transitions to `HalfOpen` and returns `true` for the probe.
    /// - `HalfOpen` → `true` (allows the probe call through)
    pub async fn is_allowed(&self) -> bool {
        // Fast path: take a read lock to check closed state.
        {
            let inner = self.inner.read().await;
            if inner.state == CircuitState::Closed {
                return true;
            }
            if inner.state == CircuitState::HalfOpen {
                return true;
            }
            // State is Open — check if timeout has elapsed.
            if let Some(last_failure) = inner.last_failure {
                if last_failure.elapsed() < self.config.timeout {
                    return false; // Still within the open window.
                }
                // Timeout elapsed — fall through to upgrade to HalfOpen.
            } else {
                // Open but no recorded failure time — should not happen; allow probe.
            }
        }

        // Upgrade to write lock to transition Open → HalfOpen.
        let mut inner = self.inner.write().await;
        // Re-check after acquiring the write lock (another task may have changed state).
        if inner.state == CircuitState::Open {
            if let Some(last_failure) = inner.last_failure {
                if last_failure.elapsed() >= self.config.timeout {
                    info!(breaker = %self.name, "circuit breaker → HalfOpen (probe)");
                    inner.state = CircuitState::HalfOpen;
                    inner.success_count = 0;
                    return true;
                }
            }
        }

        inner.state != CircuitState::Open
    }

    /// Record a successful call.
    ///
    /// In `HalfOpen` state: increments the success counter. Once `success_threshold`
    /// successes are recorded, the circuit closes.
    /// In `Closed` state: resets the failure counter.
    pub async fn record_success(&self) {
        let mut inner = self.inner.write().await;
        match inner.state {
            CircuitState::HalfOpen => {
                inner.success_count += 1;
                if inner.success_count >= self.config.success_threshold {
                    info!(breaker = %self.name, "circuit breaker → Closed (recovered)");
                    inner.state = CircuitState::Closed;
                    inner.failure_count = 0;
                    inner.success_count = 0;
                    inner.last_failure = None;
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success to prevent stale counts from opening later.
                inner.failure_count = 0;
            }
            CircuitState::Open => {
                // Ignore — no call should have been allowed while Open.
            }
        }
    }

    /// Record a failed call.
    ///
    /// In `Closed` state: increments the failure counter. Once `failure_threshold`
    /// failures are reached, the circuit opens.
    /// In `HalfOpen` state: the probe failed — reopen the circuit and reset the timeout.
    pub async fn record_failure(&self) {
        let mut inner = self.inner.write().await;
        inner.last_failure = Some(Instant::now());
        match inner.state {
            CircuitState::Closed => {
                inner.failure_count += 1;
                if inner.failure_count >= self.config.failure_threshold {
                    warn!(
                        breaker = %self.name,
                        failures = inner.failure_count,
                        "circuit breaker → Open (threshold reached)"
                    );
                    inner.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Probe failed — reopen the circuit.
                warn!(breaker = %self.name, "circuit breaker → Open (probe failed)");
                inner.state = CircuitState::Open;
                inner.success_count = 0;
            }
            CircuitState::Open => {
                // Already open — just update the last failure timestamp (already done above).
            }
        }
    }

    /// Return the current circuit state.
    pub async fn state(&self) -> CircuitState {
        self.inner.read().await.state.clone()
    }

    /// Return the current failure count (useful for metrics/diagnostics).
    pub async fn failure_count(&self) -> u32 {
        self.inner.read().await.failure_count
    }

    /// Return the breaker name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Force the circuit into the closed state (e.g., after a successful health check).
    ///
    /// Use this from the recovery manager when a provider is confirmed healthy.
    pub async fn force_close(&self) {
        let mut inner = self.inner.write().await;
        if inner.state != CircuitState::Closed {
            info!(breaker = %self.name, "circuit breaker force-closed by recovery manager");
            inner.state = CircuitState::Closed;
            inner.failure_count = 0;
            inner.success_count = 0;
            inner.last_failure = None;
        }
    }
}

impl std::fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreaker")
            .field("name", &self.name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn fast_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_millis(50),
        }
    }

    #[tokio::test]
    async fn starts_closed() {
        let cb = CircuitBreaker::new("test", fast_config());
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.is_allowed().await);
    }

    #[tokio::test]
    async fn opens_after_threshold_failures() {
        let cb = CircuitBreaker::new("test", fast_config());
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Closed); // Not yet
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(!cb.is_allowed().await);
    }

    #[tokio::test]
    async fn transitions_to_half_open_after_timeout() {
        let cb = CircuitBreaker::new("test", fast_config());
        for _ in 0..3 {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout to elapse.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // is_allowed should transition to HalfOpen.
        assert!(cb.is_allowed().await);
        assert_eq!(cb.state().await, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn closes_after_success_threshold_in_half_open() {
        let cb = CircuitBreaker::new("test", fast_config());
        // Open the circuit.
        for _ in 0..3 {
            cb.record_failure().await;
        }
        // Wait for timeout.
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Probe allowed.
        assert!(cb.is_allowed().await);
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // Record successes.
        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::HalfOpen); // 1 of 2
        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed); // 2 of 2 — closed!
    }

    #[tokio::test]
    async fn reopens_on_probe_failure() {
        let cb = CircuitBreaker::new("test", fast_config());
        for _ in 0..3 {
            cb.record_failure().await;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(cb.is_allowed().await); // Probe
        cb.record_failure().await; // Probe failed
        assert_eq!(cb.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn success_resets_failure_count_in_closed() {
        let cb = CircuitBreaker::new("test", fast_config());
        cb.record_failure().await;
        cb.record_failure().await;
        cb.record_success().await; // Should reset
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert_eq!(cb.failure_count().await, 0);
    }

    #[tokio::test]
    async fn force_close_resets_open_circuit() {
        let cb = CircuitBreaker::new("test", fast_config());
        for _ in 0..3 {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);
        cb.force_close().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.is_allowed().await);
    }
}
