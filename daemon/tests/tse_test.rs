//! Phase 43b: Task State Engine integration tests.
//!
//! These tests exercise the full event-sourced pipeline:
//!   TaskEventLog → reducer → CheckpointManager → ReplayEngine
//!
//! All tests use tempfile directories — no daemon process required.

use chrono::Utc;
use clawd::tasks::{
    checkpoint::CheckpointManager,
    event_log::TaskEventLog,
    events::{new_correlation_id, TaskEventKind},
    reducer::{self, check_write_allowed, MaterializedTask, TaskState},
    replay::ReplayEngine,
    schema::{Priority, RiskLevel, TaskSpec},
};
use std::collections::HashSet;
use tempfile::TempDir;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_spec(id: &str) -> TaskSpec {
    TaskSpec {
        id: id.to_string(),
        title: format!("Test task {}", id),
        repo: "/tmp/test-repo".to_string(),
        summary: Some("Integration test task".to_string()),
        acceptance_criteria: vec!["It works".to_string()],
        test_plan: Some("Run the tests".to_string()),
        risk_level: RiskLevel::Low,
        priority: Priority::Medium,
        labels: vec!["test".to_string()],
        owner: Some("agent-test".to_string()),
        worktree_path: None,
        worktree_branch: None,
        created_at: Utc::now(),
    }
}

async fn bootstrap_task(dir: &TempDir, task_id: &str) -> ReplayEngine {
    let engine = ReplayEngine::new(task_id, dir.path()).unwrap();
    let spec = make_spec(task_id);
    engine
        .event_log
        .append(
            TaskEventKind::TaskCreated { spec },
            "daemon",
            &new_correlation_id(),
        )
        .await
        .unwrap();
    engine
}

// ─── Test 1: Full task lifecycle ──────────────────────────────────────────────

/// Tests the canonical happy path:
///   Pending → Claimed → Active → CodeReview → Qa → Done
#[tokio::test]
async fn test_task_lifecycle_create_claim_active_done() {
    let dir = TempDir::new().unwrap();
    let task_id = "lifecycle-001";
    let engine = bootstrap_task(&dir, task_id).await;

    // Claim the task
    engine
        .event_log
        .append(
            TaskEventKind::TaskClaimed {
                agent_id: "agent-alpha".to_string(),
                role: "implementer".to_string(),
            },
            "daemon",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    // Transition to Active
    engine
        .event_log
        .append(TaskEventKind::TaskActive, "daemon", &new_correlation_id())
        .await
        .unwrap();

    let state = engine.replay().await.unwrap();
    assert_eq!(state.state, TaskState::Active);
    assert_eq!(state.claimed_by, Some("agent-alpha".to_string()));

    // File edits should now be allowed
    assert!(check_write_allowed(&state).is_ok());

    // Transition to CodeReview
    engine
        .event_log
        .append(
            TaskEventKind::TaskCodeReview {
                reviewer_id: Some("agent-reviewer".to_string()),
            },
            "daemon",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    // CodeReview → QA
    engine
        .event_log
        .append(
            TaskEventKind::TaskQa {
                qa_agent_id: Some("agent-qa".to_string()),
            },
            "daemon",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    // QA → Done
    engine
        .event_log
        .append(
            TaskEventKind::TaskDone {
                completion_notes: "All tests pass. Implementation complete.".to_string(),
            },
            "daemon",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    let final_state = engine.replay().await.unwrap();
    assert_eq!(final_state.state, TaskState::Done);

    // Verify event count
    let count = engine.event_log.event_count().await.unwrap();
    assert_eq!(count, 6); // Created, Claimed, Active, CodeReview, Qa, Done
}

// ─── Test 2: Idempotent tool calls ───────────────────────────────────────────

/// ToolCalled events with the same idempotency_key must be skipped by the reducer.
/// The state (seen_idempotency_keys) must only have the key once.
#[tokio::test]
async fn test_idempotent_tool_call() {
    let dir = TempDir::new().unwrap();
    let task_id = "idempotent-001";
    let engine = bootstrap_task(&dir, task_id).await;

    // Move to Active first
    engine
        .event_log
        .append(TaskEventKind::TaskActive, "daemon", &new_correlation_id())
        .await
        .unwrap();

    let idem_key = "tool-call-key-abc123";

    // First ToolCalled
    engine
        .event_log
        .append(
            TaskEventKind::ToolCalled {
                tool_name: "read_file".to_string(),
                arguments_hash: "hash-001".to_string(),
                idempotency_key: idem_key.to_string(),
            },
            "agent-alpha",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    // Second ToolCalled with SAME key (duplicate from retry)
    engine
        .event_log
        .append(
            TaskEventKind::ToolCalled {
                tool_name: "read_file".to_string(),
                arguments_hash: "hash-001".to_string(),
                idempotency_key: idem_key.to_string(),
            },
            "agent-alpha",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    let state = engine.replay().await.unwrap();
    assert_eq!(state.state, TaskState::Active, "state should remain Active");

    // Key should be in the set exactly once (HashSet deduplication)
    assert!(
        state.seen_idempotency_keys.contains(idem_key),
        "idempotency key should be tracked"
    );
    assert_eq!(
        state.seen_idempotency_keys.len(),
        1,
        "only one unique key should be tracked"
    );
}

// ─── Test 3: Checkpoint and replay ───────────────────────────────────────────

/// Save a checkpoint mid-stream, append more events, verify replay uses the
/// checkpoint as the starting point and still reaches the correct final state.
#[tokio::test]
async fn test_checkpoint_and_replay() {
    let dir = TempDir::new().unwrap();
    let task_id = "checkpoint-001";
    let engine = bootstrap_task(&dir, task_id).await;

    // Get to Active state
    engine
        .event_log
        .append(TaskEventKind::TaskActive, "daemon", &new_correlation_id())
        .await
        .unwrap();

    // Replay and save a checkpoint at this point
    let state_before_checkpoint = engine.replay().await.unwrap();
    assert_eq!(state_before_checkpoint.state, TaskState::Active);

    let checkpoint_mgr = CheckpointManager::new(task_id, dir.path());
    checkpoint_mgr
        .save(&state_before_checkpoint, state_before_checkpoint.event_seq)
        .await
        .unwrap();

    // Append more events after the checkpoint
    engine
        .event_log
        .append(
            TaskEventKind::TaskCodeReview { reviewer_id: None },
            "daemon",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    engine
        .event_log
        .append(
            TaskEventKind::TaskQa { qa_agent_id: None },
            "daemon",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    // Replay should load checkpoint + apply last 2 events
    let final_state = engine.replay().await.unwrap();
    assert_eq!(
        final_state.state,
        TaskState::Qa,
        "should reach Qa state after replaying from checkpoint"
    );
}

// ─── Test 4: No edit without Active+Claimed ───────────────────────────────────

/// check_write_allowed must reject edits when task is not Active+Claimed.
#[tokio::test]
async fn test_no_edit_without_active_claimed() {
    let spec = make_spec("write-guard-001");

    // Pending, no claim
    let pending = MaterializedTask::initial(spec.clone());
    assert!(
        check_write_allowed(&pending).is_err(),
        "Pending state must not allow edits"
    );

    // Active but not claimed
    let mut active_unclaimed = MaterializedTask::initial(spec.clone());
    active_unclaimed.state = TaskState::Active;
    active_unclaimed.claimed_by = None;
    assert!(
        check_write_allowed(&active_unclaimed).is_err(),
        "Active without claimed_by must not allow edits"
    );

    // Claimed but Planned (not Active)
    let mut planned_claimed = MaterializedTask::initial(spec.clone());
    planned_claimed.state = TaskState::Planned;
    planned_claimed.claimed_by = Some("agent-x".to_string());
    assert!(
        check_write_allowed(&planned_claimed).is_err(),
        "Planned state must not allow edits"
    );

    // Active + Claimed — must pass
    let mut active_claimed = MaterializedTask::initial(spec.clone());
    active_claimed.state = TaskState::Active;
    active_claimed.claimed_by = Some("agent-x".to_string());
    assert!(
        check_write_allowed(&active_claimed).is_ok(),
        "Active+Claimed must allow edits"
    );
}

// ─── Test 5: Invalid state transitions are rejected ──────────────────────────

#[tokio::test]
async fn test_invalid_transitions_rejected() {
    let spec = make_spec("invalid-001");
    let state = MaterializedTask::initial(spec);

    // Cannot go directly to Done from Pending
    use clawd::tasks::events::TaskEvent;
    let event = TaskEvent::new(
        "invalid-001",
        0,
        "daemon",
        "corr-test",
        TaskEventKind::TaskDone {
            completion_notes: "shortcut".to_string(),
        },
    );
    let result = reducer::reduce(state.clone(), &event);
    assert!(result.is_err(), "Pending → Done must be rejected");

    // Cannot go to Qa from Pending
    let event2 = TaskEvent::new(
        "invalid-001",
        0,
        "daemon",
        "corr-test",
        TaskEventKind::TaskQa { qa_agent_id: None },
    );
    let result2 = reducer::reduce(state, &event2);
    assert!(result2.is_err(), "Pending → Qa must be rejected");
}

// ─── Test 6: Blocked → Active (retry) ────────────────────────────────────────

#[tokio::test]
async fn test_blocked_retry() {
    let dir = TempDir::new().unwrap();
    let task_id = "blocked-retry-001";
    let engine = bootstrap_task(&dir, task_id).await;

    // Active
    engine
        .event_log
        .append(TaskEventKind::TaskActive, "daemon", &new_correlation_id())
        .await
        .unwrap();

    // Block it
    engine
        .event_log
        .append(
            TaskEventKind::TaskBlocked {
                reason: "Waiting for external API".to_string(),
                retry_after: None,
            },
            "daemon",
            &new_correlation_id(),
        )
        .await
        .unwrap();

    let blocked_state = engine.replay().await.unwrap();
    assert_eq!(blocked_state.state, TaskState::Blocked);

    // Unblock it (TaskActive from Blocked)
    engine
        .event_log
        .append(TaskEventKind::TaskActive, "daemon", &new_correlation_id())
        .await
        .unwrap();

    let resumed_state = engine.replay().await.unwrap();
    assert_eq!(
        resumed_state.state,
        TaskState::Active,
        "Blocked → Active retry must work"
    );
}

// ─── Test 7: Approval flow ────────────────────────────────────────────────────

#[tokio::test]
async fn test_approval_grant_and_deny() {
    // Test grant path
    {
        let dir = TempDir::new().unwrap();
        let task_id = "approval-grant-001";
        let engine = bootstrap_task(&dir, task_id).await;
        engine
            .event_log
            .append(TaskEventKind::TaskActive, "daemon", &new_correlation_id())
            .await
            .unwrap();
        engine
            .event_log
            .append(
                TaskEventKind::TaskNeedsApproval {
                    approval_id: "appr-001".to_string(),
                    tool_name: "shell_exec".to_string(),
                    risk_level: "high".to_string(),
                },
                "daemon",
                &new_correlation_id(),
            )
            .await
            .unwrap();
        engine
            .event_log
            .append(
                TaskEventKind::ApprovalGranted {
                    approval_id: "appr-001".to_string(),
                    granted_by: "user".to_string(),
                },
                "user",
                &new_correlation_id(),
            )
            .await
            .unwrap();
        let state = engine.replay().await.unwrap();
        assert_eq!(state.state, TaskState::Active, "after grant → Active");
        assert!(state.pending_approval_id.is_none(), "approval cleared");
    }

    // Test deny path
    {
        let dir = TempDir::new().unwrap();
        let task_id = "approval-deny-001";
        let engine = bootstrap_task(&dir, task_id).await;
        engine
            .event_log
            .append(TaskEventKind::TaskActive, "daemon", &new_correlation_id())
            .await
            .unwrap();
        engine
            .event_log
            .append(
                TaskEventKind::TaskNeedsApproval {
                    approval_id: "appr-002".to_string(),
                    tool_name: "git_push".to_string(),
                    risk_level: "critical".to_string(),
                },
                "daemon",
                &new_correlation_id(),
            )
            .await
            .unwrap();
        engine
            .event_log
            .append(
                TaskEventKind::ApprovalDenied {
                    approval_id: "appr-002".to_string(),
                    denied_by: "user".to_string(),
                    reason: "Too risky right now".to_string(),
                },
                "user",
                &new_correlation_id(),
            )
            .await
            .unwrap();
        let state = engine.replay().await.unwrap();
        assert_eq!(state.state, TaskState::Blocked, "after deny → Blocked");
    }
}

// ─── Test 8: claw_init idempotency ───────────────────────────────────────────

#[tokio::test]
async fn test_claw_init_idempotent() {
    let dir = TempDir::new().unwrap();
    clawd::claw_init::init_claw_dir(dir.path()).await.unwrap();
    clawd::claw_init::init_claw_dir(dir.path()).await.unwrap();

    let missing = clawd::claw_init::validate_claw_dir(dir.path()).await;
    assert!(
        missing.is_empty(),
        "no missing items after double-init: {:?}",
        missing
    );
}
