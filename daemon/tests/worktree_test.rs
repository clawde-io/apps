//! Integration tests for the task-scoped worktree manager.
//! WI.T17: Full lifecycle integration test — create → change → diff → accept.

use tempfile::TempDir;

/// Create a minimal bare-bones git repository suitable for worktree tests.
fn init_test_repo(dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let repo = git2::Repository::init(dir)?;

    // Need at least one commit before we can create branches/worktrees.
    let sig = git2::Signature::now("Test", "test@example.com")?;
    let tree_id = {
        // Write a placeholder file directly via tree builder.
        let blob = repo.blob(b"initial")?;
        let mut tb = repo.treebuilder(None)?;
        tb.insert("README", blob, 0o100644)?;
        tb.write()?
    };
    let tree = repo.find_tree(tree_id)?;
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;

    Ok(())
}

#[tokio::test]
async fn test_create_and_list_worktree() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    init_test_repo(&repo_dir).expect("init repo");

    let data_dir = tmp.path().join("data");
    let manager = clawd::worktree::WorktreeManager::new(&data_dir);

    // Create a worktree for a task.
    let info = manager
        .create("task-abc", "Fix login bug", &repo_dir)
        .await
        .expect("create worktree");

    assert_eq!(info.task_id, "task-abc");
    assert!(info.branch.starts_with("claw/task-abc-"));
    assert!(
        info.worktree_path.exists(),
        "worktree directory should exist"
    );

    // List should contain exactly one entry.
    let list = manager.list().await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].task_id, "task-abc");

    // is_in_worktree: a path inside the worktree should be detected.
    let inner_path = info.worktree_path.join("README");
    let found = manager.is_in_worktree(&inner_path).await;
    assert_eq!(found.as_deref(), Some("task-abc"));

    // is_in_worktree: a path outside should return None.
    let outside = repo_dir.join("README");
    let not_found = manager.is_in_worktree(&outside).await;
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_write_path_validation() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    init_test_repo(&repo_dir).expect("init repo");

    let data_dir = tmp.path().join("data");
    let manager = clawd::worktree::WorktreeManager::new(&data_dir);

    let info = manager
        .create("task-xyz", "Refactor", &repo_dir)
        .await
        .expect("create worktree");

    // Valid path — inside the worktree.
    let valid_path = vec![info.worktree_path.join("src").join("main.rs")];
    let result = manager.validate_write_paths("task-xyz", &valid_path).await;
    assert!(result.is_ok(), "path inside worktree should be valid");

    // Invalid path — outside the worktree (targets main workspace).
    let invalid_path = vec![repo_dir.join("src").join("main.rs")];
    let result = manager
        .validate_write_paths("task-xyz", &invalid_path)
        .await;
    assert!(result.is_err(), "path outside worktree should be rejected");

    // Unknown task_id — should error.
    let result = manager
        .validate_write_paths("nonexistent", &valid_path)
        .await;
    assert!(result.is_err(), "unknown task should return error");
}

#[tokio::test]
async fn test_bind_task_idempotent() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    init_test_repo(&repo_dir).expect("init repo");

    let data_dir = tmp.path().join("data");
    let manager = clawd::worktree::WorktreeManager::new(&data_dir);

    // First bind creates a new worktree.
    let info1 = manager
        .bind_task("task-123", "My Task", &repo_dir)
        .await
        .expect("first bind");

    // Second bind returns the same worktree (idempotent).
    let info2 = manager
        .bind_task("task-123", "My Task", &repo_dir)
        .await
        .expect("second bind");

    assert_eq!(info1.worktree_path, info2.worktree_path);
    assert_eq!(info1.branch, info2.branch);

    // Still only one worktree in the list.
    assert_eq!(manager.list().await.len(), 1);
}

#[tokio::test]
async fn test_remove_worktree() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    init_test_repo(&repo_dir).expect("init repo");

    let data_dir = tmp.path().join("data");
    let manager = clawd::worktree::WorktreeManager::new(&data_dir);

    let info = manager
        .create("task-rem", "Remove me", &repo_dir)
        .await
        .expect("create worktree");

    let wt_path = info.worktree_path.clone();
    assert!(wt_path.exists(), "worktree dir should exist before remove");

    let removed = manager.remove("task-rem").await.expect("remove");
    assert!(removed, "remove should return true for known task");

    // Worktree should no longer be in the list.
    assert_eq!(manager.list().await.len(), 0);

    // Remove non-existent should return false (not error).
    let not_found = manager.remove("nonexistent").await.expect("remove nonexistent");
    assert!(!not_found, "remove of unknown task should return false");
}

#[tokio::test]
async fn test_status_transitions() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    init_test_repo(&repo_dir).expect("init repo");

    let data_dir = tmp.path().join("data");
    let manager = clawd::worktree::WorktreeManager::new(&data_dir);

    let _info = manager
        .create("task-st", "Status test", &repo_dir)
        .await
        .expect("create worktree");

    use clawd::worktree::manager::WorktreeStatus;

    // Initial status is Active.
    let wt = manager.get("task-st").await.expect("get");
    assert_eq!(wt.status, WorktreeStatus::Active);

    // Transition to Done.
    manager.set_status("task-st", WorktreeStatus::Done).await;
    let wt = manager.get("task-st").await.expect("get after done");
    assert_eq!(wt.status, WorktreeStatus::Done);

    // Transition to Merged.
    manager.set_status("task-st", WorktreeStatus::Merged).await;
    let wt = manager.get("task-st").await.expect("get after merged");
    assert_eq!(wt.status, WorktreeStatus::Merged);
}

#[tokio::test]
async fn test_merge_requires_done_status() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    init_test_repo(&repo_dir).expect("init repo");

    let data_dir = tmp.path().join("data");
    let manager = clawd::worktree::WorktreeManager::new(&data_dir);
    let manager = std::sync::Arc::new(manager);

    let _info = manager
        .create("task-mg", "Merge test", &repo_dir)
        .await
        .expect("create worktree");

    // Attempt merge when still in Active state — should fail.
    let result = clawd::worktree::merge::merge_to_main(&manager, "task-mg").await;
    assert!(result.is_err(), "merge should fail when status != Done");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("Done"),
        "error should mention Done state: {err_msg}"
    );
}

// ─── WI.T17 — Full lifecycle integration test ─────────────────────────────

/// WI.T17: Full worktree lifecycle — create → file change → diff → accept → verify merge.
///
/// Covers the happy path end-to-end:
///   1. Create a worktree for a task.
///   2. Write a new file into the worktree directory (simulates agent code change).
///   3. Commit the change via WorktreeManager (simulates worktrees.commit RPC).
///   4. Get the staged diff via merge::stage_for_merge (used by worktrees.diff).
///   5. Verify the diff contains the expected file change.
///   6. Accept (merge) the worktree into the main branch.
///   7. Verify the file is present in the main branch HEAD tree.
///   8. Verify the worktree status is Merged.
#[tokio::test]
async fn test_full_worktree_lifecycle() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    init_test_repo(&repo_dir).expect("init repo");

    let data_dir = tmp.path().join("data");
    let manager = std::sync::Arc::new(clawd::worktree::WorktreeManager::new(&data_dir));

    // ── Step 1: Create a worktree for a task ─────────────────────────────────
    let info = manager
        .create("task-lifecycle", "Lifecycle test task", &repo_dir)
        .await
        .expect("create worktree");

    assert_eq!(info.task_id, "task-lifecycle");
    assert!(info.worktree_path.exists(), "worktree dir should exist");

    // ── Step 2: Write a new file into the worktree ───────────────────────────
    let new_file = info.worktree_path.join("feature.rs");
    std::fs::write(&new_file, b"pub fn hello() -> &'static str { \"world\" }\n")
        .expect("write new file");

    // ── Step 3: Commit the change in the worktree ────────────────────────────
    let commit_sha = {
        let wt_path = info.worktree_path.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let repo = git2::Repository::open(&wt_path)?;
            let mut index = repo.index()?;
            index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
            index.write()?;
            let tree_oid = index.write_tree()?;
            let tree = repo.find_tree(tree_oid)?;
            let sig = git2::Signature::now("Test Agent", "agent@example.com")?;
            let parents: Vec<git2::Commit<'_>> = if let Ok(head) = repo.head() {
                if let Ok(c) = head.peel_to_commit() { vec![c] } else { vec![] }
            } else {
                vec![]
            };
            let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
            let oid = repo.commit(Some("HEAD"), &sig, &sig, "Add feature.rs", &tree, &parent_refs)?;
            Ok(oid.to_string())
        })
        .await
        .expect("commit task did not panic")
        .expect("commit succeeded")
    };
    assert!(!commit_sha.is_empty(), "commit sha should be non-empty");

    // ── Step 4: Get the diff via stage_for_merge ─────────────────────────────
    let diff = clawd::worktree::merge::stage_for_merge(&manager, "task-lifecycle")
        .await
        .expect("stage_for_merge should succeed");

    // ── Step 5: Verify diff contains the new file ────────────────────────────
    assert!(
        diff.contains("feature.rs"),
        "diff should reference feature.rs; got: {diff}"
    );
    assert!(
        diff.contains("hello"),
        "diff should contain the function name; got: {diff}"
    );

    // ── Step 6: Accept (merge) the worktree ──────────────────────────────────
    // Must set status to Done first (invariant enforced by merge_to_main).
    manager
        .set_status("task-lifecycle", clawd::worktree::manager::WorktreeStatus::Done)
        .await;

    clawd::worktree::merge::merge_to_main(&manager, "task-lifecycle")
        .await
        .expect("merge should succeed");

    // ── Step 7: Verify file is present in main branch HEAD ───────────────────
    {
        let repo_dir_clone = repo_dir.clone();
        tokio::task::spawn_blocking(move || {
            let repo = git2::Repository::open(&repo_dir_clone).expect("open main repo");
            let head = repo.head().expect("get HEAD");
            let commit = head.peel_to_commit().expect("peel to commit");
            let tree = commit.tree().expect("get HEAD tree");
            assert!(
                tree.get_name("feature.rs").is_some(),
                "feature.rs should be present in main branch HEAD tree after merge"
            );
        })
        .await
        .expect("verification task did not panic");
    }

    // ── Step 8: Verify worktree status is Merged ─────────────────────────────
    let updated = manager
        .get("task-lifecycle")
        .await
        .expect("worktree should still be in registry after merge");
    assert_eq!(
        updated.status,
        clawd::worktree::manager::WorktreeStatus::Merged,
        "status should be Merged after accept"
    );
}

/// WI.T17b: Reject lifecycle — create → reject → verify cleanup.
#[tokio::test]
async fn test_worktree_reject_lifecycle() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    init_test_repo(&repo_dir).expect("init repo");

    let data_dir = tmp.path().join("data");
    let manager = clawd::worktree::WorktreeManager::new(&data_dir);

    let info = manager
        .create("task-reject", "Reject test", &repo_dir)
        .await
        .expect("create worktree");

    let wt_path = info.worktree_path.clone();
    assert!(wt_path.exists(), "worktree dir should exist before reject");

    // Write a file to confirm there's something to reject.
    std::fs::write(wt_path.join("draft.txt"), b"work in progress").unwrap();

    // Reject: remove the worktree.
    let removed = manager.remove("task-reject").await.expect("remove");
    assert!(removed, "remove should return true for known task");

    // After reject, worktree is gone from registry.
    assert!(
        manager.get("task-reject").await.is_none(),
        "worktree should be removed from registry after reject"
    );
    // List should be empty.
    assert_eq!(manager.list().await.len(), 0);
}
