//! Priority-ordered scheduling queue for provider requests.
//!
//! Requests are dequeued in descending priority order (higher `priority` value
//! = more urgent). Ties are broken by enqueue time (FIFO).

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// ── Request ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerRequest {
    /// Unique request identifier (UUID v4).
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    /// Agent role hint (e.g. `"router"`, `"planner"`, `"implementer"`, `"reviewer"`).
    pub role: String,
    /// Desired provider (e.g. `"claude"`, `"codex"`).
    pub provider: String,
    /// Priority 0–255; higher value = more urgent.
    pub priority: u8,
    pub enqueued_at: DateTime<Utc>,
}

impl Ord for SchedulerRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap: cmp returning Greater means self pops first.
        // Higher priority value → pops first.
        self.priority
            .cmp(&other.priority)
            // FIFO within the same priority tier: earlier enqueued → pops first.
            .then(other.enqueued_at.cmp(&self.enqueued_at))
    }
}

impl PartialOrd for SchedulerRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SchedulerRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for SchedulerRequest {}

// ── Queue ────────────────────────────────────────────────────────────────────

pub struct SchedulerQueue {
    queue: Mutex<BinaryHeap<SchedulerRequest>>,
}

impl SchedulerQueue {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(BinaryHeap::new()),
        }
    }

    /// Enqueue a request. Higher priority requests will be dequeued first.
    pub async fn enqueue(&self, req: SchedulerRequest) {
        self.queue.lock().await.push(req);
    }

    /// Dequeue the highest-priority request (or oldest request at equal priority).
    pub async fn dequeue(&self) -> Option<SchedulerRequest> {
        self.queue.lock().await.pop()
    }

    /// Current queue depth.
    pub async fn len(&self) -> usize {
        self.queue.lock().await.len()
    }

    /// Priority of the next request to be dequeued, or `None` if empty.
    pub async fn peek_priority(&self) -> Option<u8> {
        self.queue.lock().await.peek().map(|r| r.priority)
    }

    /// Returns `true` if the queue is empty.
    pub async fn is_empty(&self) -> bool {
        self.queue.lock().await.is_empty()
    }
}

impl Default for SchedulerQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for use in `AppContext`.
pub type SharedSchedulerQueue = Arc<SchedulerQueue>;
