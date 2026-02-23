//! Agent lifecycle tracking — registry of spawned agents (Phase 43e).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agents::capabilities::Provider;
use crate::agents::roles::AgentRole;

/// Current lifecycle state of a spawned agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Crashed,
}

/// A single agent instance tracked in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub agent_id: String,
    pub role: AgentRole,
    pub task_id: String,
    pub provider: Provider,
    pub model: String,
    pub worktree_path: Option<String>,
    pub status: AgentStatus,
    pub created_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub tokens_used: u64,
    pub cost_usd_est: f64,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// In-memory registry of all agents spawned in this daemon session.
pub struct AgentRegistry {
    agents: HashMap<String, AgentRecord>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Register a new agent record.
    pub fn register(&mut self, record: AgentRecord) {
        self.agents.insert(record.agent_id.clone(), record);
    }

    /// Update the heartbeat timestamp for an agent. Returns false if not found.
    pub fn heartbeat(&mut self, agent_id: &str) -> bool {
        if let Some(r) = self.agents.get_mut(agent_id) {
            r.last_heartbeat = Utc::now();
            true
        } else {
            false
        }
    }

    /// Update the status for an agent. Returns false if not found.
    pub fn update_status(&mut self, agent_id: &str, status: AgentStatus) -> bool {
        if let Some(r) = self.agents.get_mut(agent_id) {
            r.status = status;
            true
        } else {
            false
        }
    }

    /// Accumulate token usage and cost estimate for an agent.
    /// Callers pass per-heartbeat deltas; this adds to the running totals.
    pub fn update_usage(&mut self, agent_id: &str, tokens: u64, cost: f64) -> bool {
        if let Some(r) = self.agents.get_mut(agent_id) {
            r.tokens_used += tokens;
            r.cost_usd_est += cost;
            true
        } else {
            false
        }
    }

    /// Get an agent record by ID.
    pub fn get(&self, agent_id: &str) -> Option<&AgentRecord> {
        self.agents.get(agent_id)
    }

    /// List all agents with Running or Pending status.
    pub fn list_active(&self) -> Vec<&AgentRecord> {
        self.agents
            .values()
            .filter(|r| matches!(r.status, AgentStatus::Running | AgentStatus::Pending))
            .collect()
    }

    /// List all agents associated with a specific task.
    pub fn list_by_task(&self, task_id: &str) -> Vec<&AgentRecord> {
        self.agents
            .values()
            .filter(|r| r.task_id == task_id)
            .collect()
    }

    /// Count Running or Pending agents of a specific role.
    pub fn count_by_role(&self, role: &AgentRole) -> usize {
        self.agents
            .values()
            .filter(|r| {
                &r.role == role
                    && matches!(r.status, AgentStatus::Running | AgentStatus::Pending)
            })
            .count()
    }

    /// Mark an agent as Completed and optionally store a result string.
    /// Returns false if the agent was not found.
    pub fn mark_completed(&mut self, agent_id: &str, result: Option<String>) -> bool {
        if let Some(r) = self.agents.get_mut(agent_id) {
            r.status = AgentStatus::Completed;
            r.result = result;
            true
        } else {
            false
        }
    }

    /// Detect agents whose last heartbeat is older than `timeout_secs`.
    /// Marks them as Crashed and returns their IDs.
    pub fn detect_crashed(&mut self, timeout_secs: i64) -> Vec<String> {
        let now = Utc::now();
        let mut crashed = Vec::new();
        for (id, record) in self.agents.iter_mut() {
            if matches!(record.status, AgentStatus::Running) {
                let age = (now - record.last_heartbeat).num_seconds();
                if age > timeout_secs {
                    record.status = AgentStatus::Crashed;
                    crashed.push(id.clone());
                }
            }
        }
        crashed
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe shared registry.
pub type SharedAgentRegistry = Arc<RwLock<AgentRegistry>>;
