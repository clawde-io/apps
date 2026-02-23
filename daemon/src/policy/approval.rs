//! Human-approval request / grant / deny lifecycle.
//!
//! When the policy engine returns `PolicyDecision::NeedsApproval`, callers
//! create an `ApprovalRequest` via `ApprovalRouter::request_approval` and then
//! block on `wait_for_decision` until a human (or orchestrator) calls
//! `grant` or `deny`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::tasks::schema::RiskLevel;

// ─── Approval types ───────────────────────────────────────────────────────────

/// Current status of a pending approval request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Granted,
    Denied,
    TimedOut,
}

/// A single approval request awaiting a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Stable unique ID for this request.
    pub id: String,
    /// Task this approval is associated with.
    pub task_id: String,
    /// Agent requesting approval.
    pub agent_id: String,
    /// Tool being gated.
    pub tool: String,
    /// Human-readable one-line summary of the arguments.
    pub args_summary: String,
    /// Risk level that triggered this request.
    pub risk: RiskLevel,
    /// When the request was created.
    pub requested_at: DateTime<Utc>,
    /// Current status.
    pub status: ApprovalStatus,
}

// ─── Approval router ──────────────────────────────────────────────────────────

/// Manages in-flight approval requests and notifies waiters of decisions.
pub struct ApprovalRouter {
    requests: Arc<Mutex<HashMap<String, ApprovalRequest>>>,
    /// Broadcast channel — every update sends the `approval_id`.
    tx: broadcast::Sender<String>,
}

impl Default for ApprovalRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalRouter {
    /// Create a new router.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            tx,
        }
    }

    /// Submit an approval request.
    ///
    /// Returns the stable `approval_id` that callers use to poll or wait.
    pub async fn request_approval(
        &self,
        task_id: impl Into<String>,
        agent_id: impl Into<String>,
        tool: impl Into<String>,
        args_summary: impl Into<String>,
        risk: RiskLevel,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let request = ApprovalRequest {
            id: id.clone(),
            task_id: task_id.into(),
            agent_id: agent_id.into(),
            tool: tool.into(),
            args_summary: args_summary.into(),
            risk,
            requested_at: Utc::now(),
            status: ApprovalStatus::Pending,
        };

        self.requests.lock().await.insert(id.clone(), request);
        id
    }

    /// Grant an approval request.
    ///
    /// Transitions the request to `Granted` and broadcasts to all waiters.
    pub async fn grant(&self, approval_id: &str) -> anyhow::Result<()> {
        let mut requests = self.requests.lock().await;
        let req = requests
            .get_mut(approval_id)
            .ok_or_else(|| anyhow::anyhow!("approval '{}' not found", approval_id))?;

        if req.status != ApprovalStatus::Pending {
            return Err(anyhow::anyhow!(
                "approval '{}' is already in state {:?}",
                approval_id,
                req.status
            ));
        }

        req.status = ApprovalStatus::Granted;
        drop(requests);

        // Best-effort notify; no receivers is fine.
        let _ = self.tx.send(approval_id.to_string());
        Ok(())
    }

    /// Deny an approval request.
    pub async fn deny(&self, approval_id: &str, _reason: &str) -> anyhow::Result<()> {
        let mut requests = self.requests.lock().await;
        let req = requests
            .get_mut(approval_id)
            .ok_or_else(|| anyhow::anyhow!("approval '{}' not found", approval_id))?;

        if req.status != ApprovalStatus::Pending {
            return Err(anyhow::anyhow!(
                "approval '{}' is already in state {:?}",
                approval_id,
                req.status
            ));
        }

        req.status = ApprovalStatus::Denied;
        drop(requests);

        let _ = self.tx.send(approval_id.to_string());
        Ok(())
    }

    /// Block until the given approval has a non-`Pending` status or the timeout
    /// elapses.
    ///
    /// Returns `TimedOut` if the timeout expires before a decision is made.
    pub async fn wait_for_decision(
        &self,
        approval_id: &str,
        timeout: Duration,
    ) -> ApprovalStatus {
        let mut rx = self.tx.subscribe();
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            // Check current status first.
            {
                let requests = self.requests.lock().await;
                if let Some(req) = requests.get(approval_id) {
                    if req.status != ApprovalStatus::Pending {
                        return req.status.clone();
                    }
                } else {
                    // Not found — treat as denied.
                    return ApprovalStatus::Denied;
                }
            }

            // Wait for next notification or timeout.
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                // Mark timed-out.
                let mut requests = self.requests.lock().await;
                if let Some(req) = requests.get_mut(approval_id) {
                    req.status = ApprovalStatus::TimedOut;
                }
                return ApprovalStatus::TimedOut;
            }

            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(id)) if id == approval_id => {
                    // Re-check status in next loop iteration.
                }
                Ok(Ok(_)) => {
                    // Different approval — keep waiting.
                }
                Ok(Err(_)) | Err(_) => {
                    // Channel lagged or timeout — mark timed-out.
                    let mut requests = self.requests.lock().await;
                    if let Some(req) = requests.get_mut(approval_id) {
                        if req.status == ApprovalStatus::Pending {
                            req.status = ApprovalStatus::TimedOut;
                        }
                        return req.status.clone();
                    }
                    return ApprovalStatus::TimedOut;
                }
            }
        }
    }

    /// Look up an approval request by ID.
    pub async fn get(&self, approval_id: &str) -> Option<ApprovalRequest> {
        self.requests.lock().await.get(approval_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn grant_changes_status() {
        let router = ApprovalRouter::new();
        let id = router
            .request_approval("t1", "agent-1", "apply_patch", "patch summary", RiskLevel::High)
            .await;

        router.grant(&id).await.expect("grant");
        let req = router.get(&id).await.expect("request exists");
        assert_eq!(req.status, ApprovalStatus::Granted);
    }

    #[tokio::test]
    async fn deny_changes_status() {
        let router = ApprovalRouter::new();
        let id = router
            .request_approval("t1", "agent-1", "apply_patch", "patch summary", RiskLevel::High)
            .await;

        router.deny(&id, "not allowed").await.expect("deny");
        let req = router.get(&id).await.expect("request exists");
        assert_eq!(req.status, ApprovalStatus::Denied);
    }

    #[tokio::test]
    async fn wait_returns_granted() {
        let router = Arc::new(ApprovalRouter::new());
        let id = router
            .request_approval("t1", "agent-1", "apply_patch", "patch summary", RiskLevel::High)
            .await;

        let router2 = Arc::clone(&router);
        let id2 = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            router2.grant(&id2).await.expect("grant");
        });

        let status = router
            .wait_for_decision(&id, Duration::from_millis(500))
            .await;
        assert_eq!(status, ApprovalStatus::Granted);
    }

    #[tokio::test]
    async fn wait_times_out() {
        let router = ApprovalRouter::new();
        let id = router
            .request_approval("t1", "agent-1", "apply_patch", "patch summary", RiskLevel::High)
            .await;

        let status = router
            .wait_for_decision(&id, Duration::from_millis(50))
            .await;
        assert_eq!(status, ApprovalStatus::TimedOut);
    }
}
