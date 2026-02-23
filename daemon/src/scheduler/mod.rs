//! Account scheduler and provider intelligence.
//!
//! This module handles intelligent routing of AI provider requests:
//! - Account pool management (in-memory state over stored accounts)
//! - Sliding-window rate-limit tracking per account
//! - Exponential backoff with jitter for retries
//! - Provider fallback when primary is limited
//! - Cost-aware model recommendation
//! - Health-weighted round-robin account rotation
//! - Priority-ordered scheduling queue

pub mod accounts;
pub mod backoff;
pub mod cost;
pub mod fallback;
pub mod queue;
pub mod rate_limits;
pub mod rotation;

pub use accounts::{AccountEntry, AccountPool, SharedAccountPool};
pub use backoff::{backoff_sleep, next_backoff, BackoffConfig};
pub use cost::{estimate_cost, get_model_cost, recommend_model, ModelCostConfig};
pub use fallback::{FallbackConfig, FallbackEngine, SharedFallbackEngine};
pub use queue::{SchedulerQueue, SchedulerRequest, SharedSchedulerQueue};
pub use rate_limits::{parse_retry_after, RateLimitTracker, SharedRateLimitTracker};
pub use rotation::select_account;
