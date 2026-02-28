//! Background heartbeat monitor — detects crashed agents (Phase 43e).

use crate::agents::lifecycle::SharedAgentRegistry;

/// Periodically scans the agent registry for agents that have stopped sending
/// heartbeats. Crashed agents are marked as such so the orchestrator can
/// trigger recovery (wired in when Phase 43b replay is available).
pub async fn heartbeat_monitor(registry: SharedAgentRegistry, timeout_secs: i64) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    loop {
        interval.tick().await;
        let crashed = registry.write().await.detect_crashed(timeout_secs);
        for agent_id in &crashed {
            tracing::warn!(
                agent_id = %agent_id,
                timeout_secs = timeout_secs,
                "agent timed out — no heartbeat received, marked as Crashed"
            );
            // Recovery via 43b replay will be wired here in a future phase.
        }
    }
}
