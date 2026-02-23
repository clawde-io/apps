//! Agent role definitions for the ClawDE multi-agent orchestration system (Phase 43e).

use serde::{Deserialize, Serialize};

/// The five agent roles in the ClawDE multi-agent system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRole {
    /// Fast classification model. Decides thread type and task routing.
    Router,
    /// Read-only. Produces phases, tasks, acceptance criteria.
    Planner,
    /// Write-capable (in worktree only). Applies patches, runs tests.
    Implementer,
    /// Cross-model verification. Reviews diffs for quality and security.
    Reviewer,
    /// Tool-driven testing. Runs test suites, interprets failures.
    QaExecutor,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Router => "router",
            Self::Planner => "planner",
            Self::Implementer => "implementer",
            Self::Reviewer => "reviewer",
            Self::QaExecutor => "qa_executor",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "router" => Some(Self::Router),
            "planner" => Some(Self::Planner),
            "implementer" => Some(Self::Implementer),
            "reviewer" => Some(Self::Reviewer),
            "qa_executor" | "qa" => Some(Self::QaExecutor),
            _ => None,
        }
    }

    /// Max concurrent agents of this role (per task for implementers/reviewers,
    /// globally for router/planner).
    pub fn max_concurrent(&self) -> usize {
        match self {
            Self::Router => 1,
            Self::Planner => 1,
            Self::Implementer => 3,
            Self::Reviewer => 2,
            Self::QaExecutor => 2,
        }
    }

    /// Whether this role can modify files (requires Active+Claimed task state).
    pub fn can_write(&self) -> bool {
        matches!(self, Self::Implementer)
    }

    /// Preferred provider for cross-model verification.
    /// If implementer used Claude, reviewer should use Codex (and vice versa).
    pub fn preferred_provider_if_previous_was(
        &self,
        previous: &crate::agents::capabilities::Provider,
    ) -> crate::agents::capabilities::Provider {
        use crate::agents::capabilities::Provider;
        if matches!(self, Self::Reviewer) {
            match previous {
                Provider::Claude => Provider::Codex,
                Provider::Codex => Provider::Claude,
                _ => Provider::Claude,
            }
        } else {
            crate::agents::capabilities::recommend_provider(self.as_str(), "medium")
        }
    }
}
