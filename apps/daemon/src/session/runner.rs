use anyhow::Result;
use async_trait::async_trait;

/// Decision returned by tool.approve / tool.reject
#[derive(Debug, Clone)]
pub enum ToolDecision {
    Approved,
    Rejected,
}

/// Common interface for all AI provider runners.
#[async_trait]
pub trait Runner: Send + Sync {
    /// Execute one conversation turn: spawn the provider CLI, stream events
    /// (messageCreated, messageUpdated, toolCallCreated, etc.) via the
    /// broadcaster, and persist everything to storage. Callers must run
    /// this inside `tokio::spawn` — it blocks until the turn completes.
    async fn run_turn(&self, content: &str) -> Result<()>;

    /// Send a message to the running session (no-op for CLI-based runners
    /// that are driven entirely through `run_turn`).
    async fn send(&self, content: &str) -> Result<()>;

    /// Signal the runner to pause after the current turn completes.
    async fn pause(&self) -> Result<()>;

    /// Resume a paused session.
    async fn resume(&self) -> Result<()>;

    /// Shut down the runner cleanly.
    async fn stop(&self) -> Result<()>;
}
