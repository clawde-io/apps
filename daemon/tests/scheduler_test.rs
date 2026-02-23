//! Integration tests for the account scheduler.

use chrono::Utc;
use clawd::scheduler::{
    accounts::{AccountEntry, AccountPool},
    backoff::{next_backoff, BackoffConfig},
    queue::{SchedulerQueue, SchedulerRequest},
    rate_limits::SlidingWindow,
};

// ── Sliding window tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_rate_limit_sliding_window_counts_correctly() {
    let mut window = SlidingWindow::new(60, 5); // 5 RPM max
    let now = Utc::now();

    // No events yet — not limited.
    assert!(!window.is_limited(now));
    assert_eq!(window.count_in_window(now), 0);

    // Record 5 events.
    for _ in 0..5 {
        window.record_event(now);
    }
    assert!(window.is_limited(now), "should be limited at max count");
    assert_eq!(window.count_in_window(now), 5);

    // After the window expires, events should drop out.
    let future = now + chrono::Duration::seconds(61);
    assert!(!window.is_limited(future), "should not be limited after window expires");
}

#[tokio::test]
async fn test_rate_limit_time_until_reset() {
    let mut window = SlidingWindow::new(60, 2);
    let now = Utc::now();

    // Record max events.
    window.record_event(now);
    window.record_event(now);

    let reset = window.time_until_reset(now);
    assert!(reset.is_some(), "should have a reset time when limited");
    // Reset should be ~60 seconds away.
    assert!(
        reset.unwrap().num_seconds() > 0,
        "reset time should be positive"
    );

    // If not limited, no reset time.
    let mut window2 = SlidingWindow::new(60, 5);
    window2.record_event(now);
    assert!(window2.time_until_reset(now).is_none());
}

// ── Backoff tests ────────────────────────────────────────────────────────────

#[test]
fn test_backoff_progression() {
    let cfg = BackoffConfig::default(); // base 100ms, max 30s, x2

    let b0 = next_backoff(0, &cfg);
    let b1 = next_backoff(1, &cfg);
    let b5 = next_backoff(5, &cfg);
    let b20 = next_backoff(20, &cfg);

    // b0 should be near base (100ms +/- jitter)
    assert!(b0.as_millis() > 0, "attempt 0 should have positive delay");

    // General trend: later attempts have longer (or equal) backoffs.
    // Exact ordering may vary due to jitter, but b5 should be well above b0.
    assert!(
        b5.as_millis() > b0.as_millis(),
        "attempt 5 should be longer than attempt 0 (got {}ms vs {}ms)",
        b5.as_millis(),
        b0.as_millis()
    );

    // b20 should be capped near max_ms.
    let max_with_headroom = cfg.max_ms + (cfg.max_ms as f64 * cfg.jitter_fraction) as u64;
    assert!(
        b20.as_millis() as u64 <= max_with_headroom,
        "attempt 20 should not exceed max_ms+jitter ({}ms)",
        b20.as_millis()
    );

    let _ = b1; // suppress unused warning
}

#[test]
fn test_backoff_zero_attempt_is_nonzero() {
    let cfg = BackoffConfig {
        base_ms: 200,
        max_ms: 10_000,
        multiplier: 2.0,
        jitter_fraction: 0.1,
    };
    let b = next_backoff(0, &cfg);
    assert!(b.as_millis() >= 180, "base delay should be near 200ms (got {}ms)", b.as_millis());
}

// ── Queue priority ordering ───────────────────────────────────────────────────

#[tokio::test]
async fn test_queue_priority_ordering() {
    let queue = SchedulerQueue::new();

    let make_req = |id: &str, priority: u8, offset_secs: i64| SchedulerRequest {
        id: id.to_string(),
        task_id: "task-1".to_string(),
        agent_id: "agent-1".to_string(),
        role: "implementer".to_string(),
        provider: "claude".to_string(),
        priority,
        enqueued_at: Utc::now() + chrono::Duration::seconds(offset_secs),
    };

    // Enqueue in arbitrary order.
    queue.enqueue(make_req("low", 10, 0)).await;
    queue.enqueue(make_req("high", 200, 1)).await;
    queue.enqueue(make_req("medium", 100, 2)).await;

    assert_eq!(queue.len().await, 3);
    assert_eq!(queue.peek_priority().await, Some(200));

    // Dequeue should come out in descending priority order.
    let first = queue.dequeue().await.unwrap();
    assert_eq!(first.id, "high", "highest priority should dequeue first");

    let second = queue.dequeue().await.unwrap();
    assert_eq!(second.id, "medium", "medium priority should dequeue second");

    let third = queue.dequeue().await.unwrap();
    assert_eq!(third.id, "low", "lowest priority should dequeue last");

    assert!(queue.is_empty().await);
}

#[tokio::test]
async fn test_queue_fifo_within_same_priority() {
    let queue = SchedulerQueue::new();
    let base = Utc::now();

    let make_req = |id: &str, offset_secs: i64| SchedulerRequest {
        id: id.to_string(),
        task_id: "task-1".to_string(),
        agent_id: "agent-1".to_string(),
        role: "router".to_string(),
        provider: "claude".to_string(),
        priority: 50,
        enqueued_at: base + chrono::Duration::seconds(offset_secs),
    };

    // All same priority — should come out FIFO.
    queue.enqueue(make_req("first", 0)).await;
    queue.enqueue(make_req("second", 1)).await;
    queue.enqueue(make_req("third", 2)).await;

    assert_eq!(queue.dequeue().await.unwrap().id, "first");
    assert_eq!(queue.dequeue().await.unwrap().id, "second");
    assert_eq!(queue.dequeue().await.unwrap().id, "third");
}

// ── Account pool tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_account_pool_get_available() {
    let pool = AccountPool::new();

    let make_entry = |id: &str, provider: &str, available: bool| AccountEntry {
        account_id: id.to_string(),
        provider: provider.to_string(),
        vault_ref: format!("{}_KEY", id.to_uppercase()),
        is_available: available,
        blocked_until: None,
        rpm_used: 0,
        tpm_used: 0,
        total_requests: 0,
        last_used: None,
    };

    pool.register(make_entry("claude-1", "claude", true)).await;
    pool.register(make_entry("claude-2", "claude", false)).await; // unavailable
    pool.register(make_entry("codex-1", "codex", true)).await;

    // Should find the available claude account.
    let found = pool.get_available("claude").await;
    assert!(found.is_some(), "should find an available claude account");
    assert_eq!(found.unwrap().account_id, "claude-1");

    // Unavailable account should not be returned.
    pool.mark_blocked(
        "claude-1",
        Utc::now() + chrono::Duration::hours(1),
    )
    .await;
    let not_found = pool.get_available("claude").await;
    assert!(
        not_found.is_none(),
        "blocked account should not be returned"
    );
}
