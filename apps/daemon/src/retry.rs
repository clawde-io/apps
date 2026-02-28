// SPDX-License-Identifier: MIT
//! Exponential backoff retry for external calls.
//!
//! Provides [`retry_with_backoff`] — a generic async helper that retries a
//! fallible operation with exponentially increasing delays between attempts.
//!
//! # Example
//! ```rust,ignore
//! use clawd::retry::{retry_with_backoff, RetryConfig};
//!
//! let result = retry_with_backoff(&RetryConfig::default(), || async {
//!     call_external_service().await
//! })
//! .await;
//! ```

use std::time::Duration;
use tracing::{debug, warn};

/// Configuration for [`retry_with_backoff`].
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the first try).
    ///
    /// Default: 3
    pub max_attempts: u32,
    /// Delay before the second attempt.
    ///
    /// Each subsequent delay is multiplied by `multiplier`.
    /// Default: 500 ms
    pub initial_delay: Duration,
    /// Upper bound on the delay between attempts.
    ///
    /// Default: 30 s
    pub max_delay: Duration,
    /// Multiplier applied to the previous delay on each retry.
    ///
    /// Default: 2.0 (doubles each time)
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a config suitable for quick unit tests (no real waiting).
    pub fn instant() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            multiplier: 2.0,
        }
    }

    /// Create a config with a single attempt (no retries).
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            initial_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
            multiplier: 1.0,
        }
    }
}

/// Retry an async operation with exponential backoff.
///
/// Calls `f()` up to `config.max_attempts` times. On each failure, waits for
/// the computed backoff delay before trying again. The delay starts at
/// `config.initial_delay` and is multiplied by `config.multiplier` after each
/// attempt, capped at `config.max_delay`.
///
/// Returns `Ok(result)` on the first success, or `Err(last_error)` after all
/// attempts have been exhausted.
///
/// # Type parameters
/// - `F`: Closure that returns a `Future` producing `Result<T, E>`.
/// - `T`: Success value type.
/// - `E`: Error type; must implement `Debug` for logging.
///
/// # Panics
/// Panics if `config.max_attempts` is 0 (would never attempt the operation).
pub async fn retry_with_backoff<F, Fut, T, E>(config: &RetryConfig, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    assert!(
        config.max_attempts > 0,
        "RetryConfig.max_attempts must be at least 1"
    );

    let mut delay = config.initial_delay;
    let mut last_err: Option<E> = None;

    for attempt in 1..=config.max_attempts {
        match f().await {
            Ok(value) => {
                if attempt > 1 {
                    debug!(attempt, "retry succeeded");
                }
                return Ok(value);
            }
            Err(e) => {
                if attempt < config.max_attempts {
                    warn!(
                        attempt,
                        max = config.max_attempts,
                        delay_ms = delay.as_millis(),
                        err = ?e,
                        "attempt failed — retrying"
                    );
                    tokio::time::sleep(delay).await;
                    // Compute next delay: multiply and cap.
                    let next_ms = (delay.as_millis() as f64 * config.multiplier) as u128;
                    delay = Duration::from_millis(next_ms.min(config.max_delay.as_millis()) as u64);
                } else {
                    warn!(
                        attempt,
                        max = config.max_attempts,
                        err = ?e,
                        "all retry attempts exhausted"
                    );
                    last_err = Some(e);
                }
            }
        }
    }

    // Safety: the loop always assigns last_err when all attempts fail.
    Err(last_err.expect("retry loop ended without setting last_err"))
}

/// Convenience wrapper: retry with the default config.
///
/// Equivalent to `retry_with_backoff(&RetryConfig::default(), f).await`.
pub async fn retry<F, Fut, T, E>(f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    retry_with_backoff(&RetryConfig::default(), f).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn succeeds_on_first_attempt() {
        let cfg = RetryConfig::instant();
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = calls.clone();

        let result: Result<u32, String> = retry_with_backoff(&cfg, || {
            let c = calls2.clone();
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                Ok(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn succeeds_on_third_attempt() {
        let cfg = RetryConfig::instant();
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = calls.clone();

        let result: Result<u32, String> = retry_with_backoff(&cfg, || {
            let c = calls2.clone();
            async move {
                let n = c.fetch_add(1, Ordering::Relaxed) + 1;
                if n < 3 {
                    Err(format!("attempt {n} failed"))
                } else {
                    Ok(n)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 3);
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn returns_last_error_after_all_attempts() {
        let cfg = RetryConfig {
            max_attempts: 3,
            ..RetryConfig::instant()
        };
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = calls.clone();

        let result: Result<u32, String> = retry_with_backoff(&cfg, || {
            let c = calls2.clone();
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                Err("permanent error".to_string())
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "permanent error");
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn no_retry_config_does_one_attempt() {
        let cfg = RetryConfig::no_retry();
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = calls.clone();

        let _: Result<(), String> = retry_with_backoff(&cfg, || {
            let c = calls2.clone();
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                Err("fail".to_string())
            }
        })
        .await;

        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn delay_is_capped_at_max() {
        // Verify the delay calculation does not exceed max_delay.
        // We can observe this indirectly by running many attempts quickly.
        let cfg = RetryConfig {
            max_attempts: 10,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            multiplier: 10.0, // Very aggressive multiplier.
        };
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = calls.clone();

        let start = std::time::Instant::now();
        let _: Result<(), String> = retry_with_backoff(&cfg, || {
            let c = calls2.clone();
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                Err("fail".to_string())
            }
        })
        .await;

        // 10 attempts with max 5ms delay each = ≤50ms total.
        // Give it 1s of headroom for slow CI environments.
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(calls.load(Ordering::Relaxed), 10);
    }
}
