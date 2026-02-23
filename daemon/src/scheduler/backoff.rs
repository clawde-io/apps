//! Exponential backoff with jitter for provider retries.
//!
//! Formula: `min(base * multiplier^attempt, max) + uniform_jitter`
//! where jitter is `±(duration * jitter_fraction)`.

use std::time::Duration;

// ── Config ───────────────────────────────────────────────────────────────────

/// Configuration for exponential backoff.
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial backoff in milliseconds.
    pub base_ms: u64,
    /// Maximum backoff cap in milliseconds.
    pub max_ms: u64,
    /// Exponential growth multiplier per attempt.
    pub multiplier: f64,
    /// Jitter as a fraction of the computed backoff (0.0–1.0).
    pub jitter_fraction: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            base_ms: 100,
            max_ms: 30_000,
            multiplier: 2.0,
            jitter_fraction: 0.25,
        }
    }
}

// ── Computation ──────────────────────────────────────────────────────────────

/// Calculate the next backoff duration for `attempt` (0-indexed).
///
/// Returns `min(base_ms * multiplier^attempt, max_ms)` plus a random jitter
/// of `±(computed * jitter_fraction / 2)` — always non-negative.
pub fn next_backoff(attempt: u32, config: &BackoffConfig) -> Duration {
    let base = config.base_ms as f64;
    let raw = base * config.multiplier.powi(attempt as i32);
    let capped = raw.min(config.max_ms as f64);

    // Deterministic pseudo-jitter derived from attempt (avoids a rand dep).
    // Uses a simple LCG step seeded with attempt number for spread.
    let jitter_range = capped * config.jitter_fraction;
    let pseudo_random_fraction = pseudo_rand(attempt) * jitter_range;
    let with_jitter = (capped + pseudo_random_fraction).max(0.0);

    Duration::from_millis(with_jitter as u64)
}

/// Async sleep for the computed backoff duration.
pub async fn backoff_sleep(attempt: u32, config: &BackoffConfig) {
    let duration = next_backoff(attempt, config);
    tokio::time::sleep(duration).await;
}

// ── Pseudo-random helper (no external dependency) ────────────────────────────

/// Produce a float in [-0.5, 0.5) using a simple LCG seeded by `attempt`.
/// This avoids adding a `rand` dependency for a small jitter spread.
fn pseudo_rand(attempt: u32) -> f64 {
    // LCG parameters (Numerical Recipes)
    const A: u64 = 1_664_525;
    const C: u64 = 1_013_904_223;
    const M: u64 = 1u64 << 32;
    let state = A.wrapping_mul(attempt as u64).wrapping_add(C) % M;
    // Map to [-0.5, 0.5)
    (state as f64 / M as f64) - 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_increases_with_attempt() {
        let cfg = BackoffConfig::default();
        let b0 = next_backoff(0, &cfg);
        let b1 = next_backoff(1, &cfg);
        let b2 = next_backoff(2, &cfg);
        // Allow for jitter — just ensure general trend.
        assert!(
            b2 >= b0,
            "later attempt should generally have longer backoff"
        );
        let _ = b1; // used
    }

    #[test]
    fn backoff_capped_at_max() {
        let cfg = BackoffConfig::default();
        // Attempt 100 should be capped.
        let b = next_backoff(100, &cfg);
        // Allow up to max_ms + jitter_fraction * max_ms headroom.
        let max_with_jitter = cfg.max_ms + (cfg.max_ms as f64 * cfg.jitter_fraction) as u64;
        assert!(
            b.as_millis() as u64 <= max_with_jitter,
            "backoff should not greatly exceed max_ms ({}ms > {}ms)",
            b.as_millis(),
            max_with_jitter
        );
    }
}
