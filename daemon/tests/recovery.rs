//! Integration tests for session recovery on daemon restart.
//! These tests use Storage directly (no real claude CLI needed) — they run in CI.

use clawd::storage::Storage;
use std::fs;
use tempfile::TempDir;

/// Helper: create a fresh Storage in a temp dir
async fn make_storage(dir: &TempDir) -> Storage {
    Storage::new(dir.path()).await.expect("storage init failed")
}

#[tokio::test]
async fn test_stale_session_recovery_on_restart() {
    let dir = TempDir::new().unwrap();

    // 1. Create storage and session
    let storage = make_storage(&dir).await;
    let session = storage
        .create_session("claude", "/tmp/repo", "test session", None)
        .await
        .expect("create session");

    // 2. Mark it as "running" (simulates a crash mid-turn)
    storage
        .update_session_status(&session.id, "running")
        .await
        .expect("update status");

    // Verify it's running
    let s = storage.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(s.status, "running");

    // 3. Create a JSONL file to simulate a partial run
    let sessions_dir = dir.path().join("sessions");
    fs::create_dir_all(&sessions_dir).unwrap();
    let jsonl_path = sessions_dir.join(format!("{}.jsonl", session.id));
    fs::write(&jsonl_path, "{\"type\":\"partial\"}\n").unwrap();

    // 4. Simulate daemon restart: create a new Storage instance pointing at the same dir
    let storage2 = make_storage(&dir).await;

    // 5. Run recovery
    let recovered = storage2
        .recover_stale_sessions()
        .await
        .expect("recover_stale_sessions failed");
    assert_eq!(recovered, 1, "expected 1 session to be recovered");

    // 6. Verify session status is now "error"
    let s2 = storage2.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(
        s2.status, "error",
        "stale running session should become 'error'"
    );

    // 7. JSONL file should still exist (preserved on recovery)
    assert!(
        jsonl_path.exists(),
        "JSONL file should be preserved after recovery"
    );
}

#[tokio::test]
async fn test_multiple_stale_sessions_all_marked_error() {
    let dir = TempDir::new().unwrap();
    let storage = make_storage(&dir).await;

    // Create 3 sessions and set them all to "running"
    let mut ids = Vec::new();
    for i in 0..3 {
        let s = storage
            .create_session(
                "claude",
                &format!("/tmp/repo{i}"),
                &format!("session {i}"),
                None,
            )
            .await
            .expect("create session");
        storage
            .update_session_status(&s.id, "running")
            .await
            .expect("update status");
        ids.push(s.id);
    }

    // Also create one idle session — it should NOT be affected
    let idle = storage
        .create_session("claude", "/tmp/idle", "idle session", None)
        .await
        .expect("create session");
    assert_eq!(idle.status, "idle");

    // Simulate restart
    let storage2 = make_storage(&dir).await;
    let recovered = storage2
        .recover_stale_sessions()
        .await
        .expect("recover failed");
    assert_eq!(recovered, 3, "all 3 running sessions should be recovered");

    // Verify all 3 are now "error"
    for id in &ids {
        let s = storage2.get_session(id).await.unwrap().unwrap();
        assert_eq!(s.status, "error", "session {id} should be 'error'");
    }

    // Idle session should still be idle
    let idle2 = storage2.get_session(&idle.id).await.unwrap().unwrap();
    assert_eq!(idle2.status, "idle", "idle session should remain idle");
}

#[tokio::test]
async fn test_paused_session_becomes_idle_on_recovery() {
    let dir = TempDir::new().unwrap();
    let storage = make_storage(&dir).await;

    let session = storage
        .create_session("claude", "/tmp/repo", "paused session", None)
        .await
        .expect("create session");

    // Mark as paused
    storage
        .update_session_status(&session.id, "paused")
        .await
        .expect("update status");

    // Simulate restart
    let storage2 = make_storage(&dir).await;
    let recovered = storage2
        .recover_stale_sessions()
        .await
        .expect("recover failed");
    assert_eq!(recovered, 1, "paused session should be recovered");

    let s = storage2.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(
        s.status, "idle",
        "paused session should become 'idle' on recovery"
    );
}
