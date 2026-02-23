use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Canonical task specification (stored as YAML in .claw/tasks/<id>/task.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub title: String,
    pub repo: String,
    pub summary: Option<String>,
    pub acceptance_criteria: Vec<String>,
    pub test_plan: Option<String>,
    pub risk_level: RiskLevel,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub owner: Option<String>,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}
