//! Multi-agent orchestrator — spawns, tracks, and coordinates agents (Phase 43e).

use std::sync::Arc;

use crate::agents::capabilities::Provider;
use crate::agents::lifecycle::{AgentRecord, AgentRegistry, AgentStatus, SharedAgentRegistry};
use crate::agents::roles::AgentRole;
use crate::agents::routing::route_agent;

/// Orchestrates the lifecycle of all agents in the system.
///
/// Enforces concurrency caps, routes providers, and supports the standard
/// planner → implementer → reviewer → QA handoff chain.
pub struct Orchestrator {
    pub registry: SharedAgentRegistry,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(tokio::sync::RwLock::new(AgentRegistry::new())),
        }
    }

    /// Spawn an agent for the given role + task. Enforces `max_concurrent` caps.
    ///
    /// Returns the new agent ID on success.
    pub async fn spawn(
        &self,
        role: AgentRole,
        task_id: &str,
        complexity: &str,
        worktree_path: Option<String>,
        previous_provider: Option<Provider>,
    ) -> Result<String, OrchestratorError> {
        // Compute routing before acquiring the write lock (no shared state needed).
        let providers = vec![Provider::Claude, Provider::Codex];
        let decision =
            route_agent(&role, complexity, previous_provider.as_ref(), &providers);

        let agent_id = format!(
            "A-{}",
            &uuid::Uuid::new_v4().to_string()[..8]
        );
        let now = chrono::Utc::now();

        let record = AgentRecord {
            agent_id: agent_id.clone(),
            role: role.clone(),
            task_id: task_id.to_string(),
            provider: decision.provider,
            model: decision.model,
            worktree_path,
            status: AgentStatus::Pending,
            created_at: now,
            last_heartbeat: now,
            tokens_used: 0,
            cost_usd_est: 0.0,
            result: None,
            error: None,
        };

        // Hold the write lock for the entire check-then-register sequence to
        // prevent a TOCTOU race where two concurrent spawns both pass the cap
        // check on separate read locks and then both register.
        let mut registry = self.registry.write().await;
        let current_count = registry.count_by_role(&role);
        if current_count >= role.max_concurrent() {
            return Err(OrchestratorError::ConcurrencyCapReached {
                role: role.as_str().to_string(),
                limit: role.max_concurrent(),
            });
        }
        registry.register(record);
        Ok(agent_id)
    }

    /// Cancel an agent — sets its status to Failed.
    pub async fn cancel(&self, agent_id: &str) -> bool {
        self.registry
            .write()
            .await
            .update_status(agent_id, AgentStatus::Failed)
    }

    /// Pause an agent — sets its status to Paused.
    pub async fn pause(&self, agent_id: &str) -> bool {
        self.registry
            .write()
            .await
            .update_status(agent_id, AgentStatus::Paused)
    }

    /// Resume a paused agent — sets its status back to Running.
    pub async fn resume(&self, agent_id: &str) -> bool {
        self.registry
            .write()
            .await
            .update_status(agent_id, AgentStatus::Running)
    }

    /// Mark an agent as Running (transition from Pending).
    pub async fn mark_running(&self, agent_id: &str) -> bool {
        self.registry
            .write()
            .await
            .update_status(agent_id, AgentStatus::Running)
    }

    /// Mark an agent as Completed.
    pub async fn mark_completed(&self, agent_id: &str, result: Option<String>) -> bool {
        self.registry.write().await.mark_completed(agent_id, result)
    }

    /// Record a heartbeat + optional usage update.
    pub async fn heartbeat(
        &self,
        agent_id: &str,
        tokens_used: Option<u64>,
        cost_usd: Option<f64>,
    ) -> bool {
        let mut registry = self.registry.write().await;
        let found = registry.heartbeat(agent_id);
        if found {
            if let (Some(t), Some(c)) = (tokens_used, cost_usd) {
                registry.update_usage(agent_id, t, c);
            }
        }
        found
    }

    /// Run the standard handoff chain: Planner → Implementer(s) → Reviewer → QA Executor.
    ///
    /// Returns the agent IDs in order of spawning.
    pub async fn run_handoff_chain(
        &self,
        task_id: &str,
        complexity: &str,
    ) -> Result<Vec<String>, OrchestratorError> {
        let mut chain = Vec::new();

        // 1. Planner (read-only)
        let planner_id = self
            .spawn(AgentRole::Planner, task_id, complexity, None, None)
            .await?;
        chain.push(planner_id);

        // 2. Implementer — default to Claude for code generation
        let impl_id = self
            .spawn(
                AgentRole::Implementer,
                task_id,
                complexity,
                None,
                Some(Provider::Claude),
            )
            .await?;
        chain.push(impl_id);

        // 3. Reviewer — cross-model: implementer used Claude, so pass Claude as
        // previous_provider so route_agent selects Codex for the reviewer.
        let review_id = self
            .spawn(
                AgentRole::Reviewer,
                task_id,
                complexity,
                None,
                Some(Provider::Claude), // previous = Claude → reviewer routes to Codex
            )
            .await?;
        chain.push(review_id);

        // 4. QA Executor
        let qa_id = self
            .spawn(AgentRole::QaExecutor, task_id, complexity, None, None)
            .await?;
        chain.push(qa_id);

        Ok(chain)
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors returned by the orchestrator.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("concurrency cap reached for role {role}: max {limit}")]
    ConcurrencyCapReached { role: String, limit: usize },
    #[error("agent not found: {0}")]
    AgentNotFound(String),
}

/// Thread-safe shared orchestrator.
pub type SharedOrchestrator = Arc<Orchestrator>;
