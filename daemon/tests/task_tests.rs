//! Phase 41: TaskStorage unit tests — no running daemon, uses in-memory SQLite
//! via Storage::new (same migration path as production).

use clawd::storage::Storage;
use clawd::tasks::markdown_parser::ParsedTask;
use clawd::tasks::storage::{
    ActivityQueryParams, TaskListParams, TaskStorage, MISSING_COMPLETION_NOTES,
    TASK_ALREADY_CLAIMED,
};

/// Spin up a temporary Storage (SQLite on disk via tempdir) and return TaskStorage.
async fn make_ts() -> (TaskStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let storage = Storage::new(dir.path()).await.expect("Storage::new failed");
    let ts = TaskStorage::new(storage.clone_pool());
    (ts, dir)
}

// ─── 1. Atomic claim ─────────────────────────────────────────────────────────

/// Two concurrent agents race to claim the same task.
/// Exactly one must succeed and one must get TASK_ALREADY_CLAIMED.
#[tokio::test]
async fn test_atomic_claim() {
    let (ts, _dir) = make_ts().await;

    ts.add_task(
        "atomic-1",
        "Atomic claim test task",
        Some("code"),
        Some("41-test"),
        None,
        None,
        Some("high"),
        None,
        None,
        None,
        None,
        None,
        "/tmp/test-repo",
    )
    .await
    .expect("add_task failed");

    let ts_a = ts.clone();
    let ts_b = ts.clone();

    let fut_a = tokio::spawn(async move { ts_a.claim_task("atomic-1", "agent-A", None).await });
    let fut_b = tokio::spawn(async move { ts_b.claim_task("atomic-1", "agent-B", None).await });

    let (res_a, res_b) = tokio::join!(fut_a, fut_b);
    let res_a = res_a.expect("join_a panicked");
    let res_b = res_b.expect("join_b panicked");

    // Exactly one winner.
    let successes = [res_a.is_ok(), res_b.is_ok()]
        .iter()
        .filter(|&&b| b)
        .count();
    assert_eq!(successes, 1, "exactly one agent should win the claim");

    // Verify claimed_by in the DB.
    let task = ts
        .get_task("atomic-1")
        .await
        .expect("get_task failed")
        .expect("task not found");

    let claimed_by = task.claimed_by.expect("claimed_by must be set");
    assert!(
        claimed_by == "agent-A" || claimed_by == "agent-B",
        "claimed_by should be one of the two agents, got: {claimed_by}"
    );

    // The losing future must contain the TASK_ALREADY_CLAIMED error code.
    let loser_err = if let Err(e) = res_a {
        e
    } else {
        res_b.unwrap_err()
    };
    let err_str = loser_err.to_string();
    assert!(
        err_str.contains(&TASK_ALREADY_CLAIMED.to_string()),
        "losing agent error should contain TASK_ALREADY_CLAIMED ({TASK_ALREADY_CLAIMED}), got: {err_str}"
    );
}

// ─── 2. Completion notes enforcement ─────────────────────────────────────────

/// update_status("done") without notes must fail with MISSING_COMPLETION_NOTES.
/// With notes it must succeed.
#[tokio::test]
async fn test_completion_notes_enforcement() {
    let (ts, _dir) = make_ts().await;

    ts.add_task(
        "notes-1",
        "Notes enforcement task",
        Some("code"),
        Some("41-test"),
        None,
        None,
        Some("medium"),
        None,
        None,
        None,
        None,
        None,
        "/tmp/test-repo",
    )
    .await
    .expect("add_task failed");

    ts.claim_task("notes-1", "bot1", None)
        .await
        .expect("claim_task failed");

    // None notes → must fail.
    let err_none = ts.update_status("notes-1", "done", None, None).await;
    assert!(
        err_none.is_err(),
        "update_status with None notes should fail"
    );
    let code_str = MISSING_COMPLETION_NOTES.to_string();
    assert!(
        err_none.unwrap_err().to_string().contains(&code_str),
        "error should contain MISSING_COMPLETION_NOTES ({code_str})"
    );

    // Empty-string notes → must also fail.
    let err_empty = ts.update_status("notes-1", "done", Some(""), None).await;
    assert!(
        err_empty.is_err(),
        "update_status with empty notes should fail"
    );

    // Non-empty notes → must succeed.
    let done = ts
        .update_status("notes-1", "done", Some("completed feature X"), None)
        .await
        .expect("update_status with notes should succeed");

    assert_eq!(done.status, "done");
    assert_eq!(done.notes.as_deref(), Some("completed feature X"));
}

// ─── 3. Full task lifecycle ───────────────────────────────────────────────────

/// add → claim → heartbeat → update_status("done") with notes.
#[tokio::test]
async fn test_task_lifecycle() {
    let (ts, _dir) = make_ts().await;

    let added = ts
        .add_task(
            "lifecycle-1",
            "Lifecycle test task",
            Some("code"),
            Some("41-test"),
            None,
            None,
            Some("medium"),
            None,
            None,
            None,
            None,
            Some(30),
            "/tmp/test-repo",
        )
        .await
        .expect("add_task failed");

    assert_eq!(added.id, "lifecycle-1");
    assert_eq!(added.status, "pending");
    assert!(added.claimed_by.is_none());

    // Claim.
    let claimed = ts
        .claim_task("lifecycle-1", "agent-lifecycle", None)
        .await
        .expect("claim_task failed");

    assert_eq!(claimed.status, "in_progress");
    assert_eq!(claimed.claimed_by.as_deref(), Some("agent-lifecycle"));
    assert!(claimed.claimed_at.is_some(), "claimed_at must be set");

    // Heartbeat (must not error).
    ts.heartbeat_task("lifecycle-1", "agent-lifecycle")
        .await
        .expect("heartbeat_task failed");

    let after_hb = ts
        .get_task("lifecycle-1")
        .await
        .expect("get_task failed")
        .expect("task missing after heartbeat");
    assert!(
        after_hb.last_heartbeat.is_some(),
        "last_heartbeat must be set after heartbeat"
    );

    // Complete.
    let done = ts
        .update_status(
            "lifecycle-1",
            "done",
            Some("lifecycle test passed — all steps verified"),
            None,
        )
        .await
        .expect("update_status to done failed");

    assert_eq!(done.status, "done");
    assert_eq!(
        done.notes.as_deref(),
        Some("lifecycle test passed — all steps verified")
    );
    assert!(
        done.completed_at.is_some(),
        "completed_at must be set when done"
    );
}

// ─── 4. Activity log and query ────────────────────────────────────────────────

/// Log 3 entries (2 auto + 1 note) for a task, query by task_id (3), by
/// entry_type="note" (1), then post a phase-level note and query by phase.
#[tokio::test]
async fn test_activity_log_and_query() {
    let (ts, _dir) = make_ts().await;

    ts.add_task(
        "log-task-1",
        "Activity log test task",
        Some("code"),
        Some("41-test"),
        None,
        None,
        Some("medium"),
        None,
        None,
        None,
        None,
        None,
        "/tmp/test-repo",
    )
    .await
    .expect("add_task failed");

    // Two auto entries.
    ts.log_activity(
        "agent-log",
        Some("log-task-1"),
        Some("41-test"),
        "task_started",
        "auto",
        Some("task execution began"),
        None,
        "/tmp/test-repo",
    )
    .await
    .expect("log_activity auto 1 failed");

    ts.log_activity(
        "agent-log",
        Some("log-task-1"),
        Some("41-test"),
        "heartbeat",
        "auto",
        None,
        None,
        "/tmp/test-repo",
    )
    .await
    .expect("log_activity auto 2 failed");

    // One note entry.
    ts.post_note(
        "agent-log",
        Some("log-task-1"),
        Some("41-test"),
        "This is a human note for task log-task-1",
        "/tmp/test-repo",
    )
    .await
    .expect("post_note failed");

    // Query by task_id → expect 3.
    let by_task = ts
        .query_activity(&ActivityQueryParams {
            task_id: Some("log-task-1".to_string()),
            ..Default::default()
        })
        .await
        .expect("query_activity by task_id failed");

    assert_eq!(by_task.len(), 3, "expected 3 entries for log-task-1");

    // Query by task_id + entry_type=note → expect 1.
    let by_note = ts
        .query_activity(&ActivityQueryParams {
            task_id: Some("log-task-1".to_string()),
            entry_type: Some("note".to_string()),
            ..Default::default()
        })
        .await
        .expect("query_activity by entry_type failed");

    assert_eq!(by_note.len(), 1, "expected 1 note entry");
    assert_eq!(by_note[0].entry_type, "note");
    assert_eq!(
        by_note[0].detail.as_deref(),
        Some("This is a human note for task log-task-1")
    );

    // Post a phase-level note (task_id = None).
    ts.post_note(
        "agent-log",
        None,
        Some("41-test"),
        "Phase 41-test summary note (task_id = None)",
        "/tmp/test-repo",
    )
    .await
    .expect("post phase note failed");

    // Query by phase → should include task entries for phase 41-test AND the phase note.
    let by_phase = ts
        .query_activity(&ActivityQueryParams {
            phase: Some("41-test".to_string()),
            ..Default::default()
        })
        .await
        .expect("query_activity by phase failed");

    let phase_note = by_phase
        .iter()
        .find(|e| e.task_id.is_none() && e.entry_type == "note");
    assert!(
        phase_note.is_some(),
        "phase-level note (task_id=None, entry_type=note) should appear in phase query"
    );
}

// ─── 5. Backfill from tasks (idempotent) ────────────────────────────────────

/// backfill_from_tasks inserts new tasks and skips existing ones.
#[tokio::test]
async fn test_backfill_from_tasks() {
    let (ts, _dir) = make_ts().await;

    let tasks = vec![
        ParsedTask {
            id: "BF-1".to_string(),
            title: "Backfill task one".to_string(),
            severity: Some("high".to_string()),
            file: Some("src/main.rs".to_string()),
            status: "pending".to_string(),
            phase: Some("41-test".to_string()),
            group: Some("group-A".to_string()),
        },
        ParsedTask {
            id: "BF-2".to_string(),
            title: "Backfill task two".to_string(),
            severity: Some("medium".to_string()),
            file: None,
            status: "done".to_string(),
            phase: Some("41-test".to_string()),
            group: None,
        },
        ParsedTask {
            id: "BF-3".to_string(),
            title: "Backfill task three".to_string(),
            severity: None,
            file: None,
            status: "in_progress".to_string(),
            phase: None,
            group: None,
        },
    ];

    // First call: insert all 3.
    let count = ts
        .backfill_from_tasks(tasks.clone(), "/tmp/test-repo")
        .await
        .expect("backfill_from_tasks failed");

    assert_eq!(count, 3, "first backfill should insert 3 tasks");

    // Verify inserted tasks have the correct statuses.
    let bf1 = ts
        .get_task("BF-1")
        .await
        .expect("get BF-1 failed")
        .expect("BF-1 not found");
    assert_eq!(bf1.status, "pending");
    assert_eq!(bf1.phase.as_deref(), Some("41-test"));

    let bf2 = ts
        .get_task("BF-2")
        .await
        .expect("get BF-2 failed")
        .expect("BF-2 not found");
    assert_eq!(bf2.status, "done", "BF-2 should have status 'done'");

    // Second call with same data: 0 new inserts (idempotent).
    let count2 = ts
        .backfill_from_tasks(tasks, "/tmp/test-repo")
        .await
        .expect("second backfill_from_tasks failed");

    assert_eq!(
        count2, 0,
        "second backfill with same data should insert 0 tasks"
    );

    // Confirm total count is still 3 — no duplicates.
    let all = ts
        .list_tasks(&TaskListParams {
            repo_path: Some("/tmp/test-repo".to_string()),
            ..Default::default()
        })
        .await
        .expect("list_tasks failed");

    assert_eq!(
        all.len(),
        3,
        "should still have exactly 3 tasks after idempotent backfill"
    );
}
