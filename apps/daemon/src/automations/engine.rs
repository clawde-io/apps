//! Automation engine — trigger evaluation and action dispatch.
//!
//! Automations are lightweight "if trigger → run action" rules loaded from
//! `.claw/config.toml` and three built-in automations that are always active.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::AppContext;

// ─── Trigger types ─────────────────────────────────────────────────────────

/// Events that can fire an automation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    /// An AI session ended (status = completed or error).
    SessionComplete,
    /// A task's status changed to "done".
    TaskDone,
    /// A file in the repo was written by a tool call.
    FileSaved,
    /// A cron expression fired (simple: `"@hourly"`, `"@daily"`, `"*/5 * * * *"`).
    Cron,
}

/// A trigger event payload passed to automation evaluators.
#[derive(Debug, Clone)]
pub struct TriggerEvent {
    pub kind: TriggerType,
    /// Session ID that caused the event (if applicable).
    pub session_id: Option<String>,
    /// Task ID that changed (if applicable).
    pub task_id: Option<String>,
    /// File path that was saved (if applicable).
    pub file_path: Option<String>,
    /// Raw session output (last AI message) — used by TODO extractor.
    pub session_output: Option<String>,
    /// Duration of the session in seconds (for long-session notifier).
    pub session_duration_secs: Option<u64>,
}

// ─── Action types ──────────────────────────────────────────────────────────

/// Actions an automation can execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Run a shell command (e.g. `cargo test`, `npm test`).
    RunTests,
    /// Send a push event to all connected clients.
    SendNotification,
    /// Create a new task in the task store.
    CreateTask,
    /// Run an arbitrary shell script.
    RunScript,
}

// ─── Automation definition ─────────────────────────────────────────────────

/// A single automation rule (from config or built-in).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    /// Unique machine-readable name.
    pub name: String,
    /// Human-readable description shown in the UI.
    pub description: String,
    /// Whether this automation is currently active.
    pub enabled: bool,
    /// What fires this automation.
    pub trigger: TriggerType,
    /// Optional condition expression (simple key=value check on event fields).
    pub condition: Option<String>,
    /// What the automation does when triggered.
    pub action: ActionType,
    /// Action-specific configuration (command to run, task title template, etc.).
    pub action_config: serde_json::Value,
    /// Whether this is a built-in (cannot be deleted, only disabled).
    pub builtin: bool,
    /// ISO-8601 timestamp of last trigger (or None if never run).
    pub last_triggered_at: Option<String>,
}

impl Automation {
    /// Returns true if the event matches this automation's trigger.
    pub fn matches(&self, event: &TriggerEvent) -> bool {
        if !self.enabled {
            return false;
        }
        if self.trigger != event.kind {
            return false;
        }
        // Simple condition check: "key=value" string matched against event fields.
        if let Some(cond) = &self.condition {
            if !evaluate_condition(cond, event) {
                return false;
            }
        }
        true
    }
}

/// Evaluate a simple condition string like `"session_duration_secs>300"`.
fn evaluate_condition(condition: &str, event: &TriggerEvent) -> bool {
    if let Some(rest) = condition.strip_prefix("session_duration_secs>") {
        if let (Ok(threshold), Some(actual)) = (rest.parse::<u64>(), event.session_duration_secs) {
            return actual > threshold;
        }
    }
    if let Some(rest) = condition.strip_prefix("file_ext=") {
        if let Some(path) = &event.file_path {
            return path.ends_with(rest);
        }
    }
    // Unknown condition — treat as always true so the user's intent is respected.
    true
}

// ─── Automation registry ───────────────────────────────────────────────────

/// Shared automation registry (automations list + event channel).
pub struct AutomationEngine {
    /// All registered automations (built-ins + user-configured).
    pub automations: tokio::sync::RwLock<Vec<Automation>>,
    /// Channel for dispatching trigger events internally.
    pub event_tx: broadcast::Sender<TriggerEvent>,
}

impl AutomationEngine {
    pub fn new(initial: Vec<Automation>) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(64);
        Arc::new(Self {
            automations: tokio::sync::RwLock::new(initial),
            event_tx,
        })
    }

    /// Fire a trigger event. All matching automations will execute asynchronously.
    pub fn fire(&self, event: TriggerEvent) {
        if let Err(e) = self.event_tx.send(event) {
            debug!("AutomationEngine: no listeners for trigger event: {e}");
        }
    }

    /// Start the background dispatcher task. Must be called once at daemon startup.
    pub fn start_dispatcher(engine: Arc<Self>, ctx: AppContext) {
        let mut rx = engine.event_tx.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let automations = engine.automations.read().await;
                        for auto in automations.iter() {
                            if auto.matches(&event) {
                                let auto = auto.clone();
                                let ctx2 = ctx.clone();
                                let event2 = event.clone();
                                tokio::spawn(async move {
                                    if let Err(e) =
                                        crate::automations::builtins::execute(&auto, &event2, &ctx2)
                                            .await
                                    {
                                        warn!(
                                            name = %auto.name,
                                            "automation action failed: {e}"
                                        );
                                    }
                                });
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("AutomationEngine: dropped {n} events (too slow)");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

/// Convenience — get a snapshot of all automations for RPC responses.
pub async fn list_automations(engine: &AutomationEngine) -> Vec<Automation> {
    engine.automations.read().await.clone()
}
