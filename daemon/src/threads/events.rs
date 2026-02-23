//! Thread event bus — streams task thread events to control thread observers
//! (Phase 43f).
//!
//! Task threads emit events as they work. The control thread subscribes to
//! these events so it can show real-time status to the user and request
//! approvals when a high-risk tool is about to be called.
//!
//! The bus is backed by a `tokio::sync::broadcast::channel` so multiple
//! subscribers (control thread, UI push events, telemetry) can consume the
//! same stream without blocking the sender.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Capacity of the broadcast channel.
/// 256 events is sufficient — slow consumers lag and skip old events.
const BUS_CAPACITY: usize = 256;

/// Events emitted by task threads during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThreadEvent {
    /// A task thread has begun work.
    TaskStarted { task_id: String, thread_id: String },
    /// A tool was invoked inside a task thread.
    ToolCalled { tool: String, task_id: String },
    /// The task thread cannot proceed without external input.
    TaskBlocked { reason: String, task_id: String },
    /// The task thread completed its work successfully.
    TaskCompleted { task_id: String, summary: String },
    /// A high-risk tool needs human approval before proceeding.
    ApprovalNeeded {
        task_id: String,
        tool: String,
        /// Risk classification from the tool-risk policy (e.g. "high", "critical").
        risk: String,
    },
}

/// Shared broadcast bus for thread events.
///
/// Clone cheaply — the underlying `broadcast::Sender` is `Arc`-backed.
#[derive(Clone)]
pub struct ThreadEventBus {
    sender: broadcast::Sender<ThreadEvent>,
}

impl ThreadEventBus {
    /// Create a new bus with the standard capacity.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Self { sender }
    }

    /// Subscribe to the event stream.
    ///
    /// The returned receiver will receive all events emitted AFTER the call to
    /// `subscribe()`. Events emitted before subscribing are not replayed.
    pub fn subscribe(&self) -> broadcast::Receiver<ThreadEvent> {
        self.sender.subscribe()
    }

    /// Emit an event to all current subscribers.
    ///
    /// Silently drops the event if there are no subscribers (no error).
    pub fn emit(&self, event: ThreadEvent) {
        // send() errors only when there are 0 subscribers — that's fine.
        let _ = self.sender.send(event);
    }
}

impl Default for ThreadEventBus {
    fn default() -> Self {
        Self::new()
    }
}
