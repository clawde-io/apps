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
    /// Send a message to the running session. The runner is responsible for
    /// emitting events (messageCreated, messageUpdated, toolCallCreated, etc.)
    /// via the broadcaster and persisting them to storage/event log.
    async fn send(&self, content: &str) -> Result<()>;

    /// Signal the runner to pause after the current turn completes.
    async fn pause(&self) -> Result<()>;

    /// Resume a paused session.
    async fn resume(&self) -> Result<()>;

    /// Shut down the runner cleanly.
    async fn stop(&self) -> Result<()>;
}
