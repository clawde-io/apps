//! Integration tests for the task-scoped worktree manager.

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
    assert!(info.worktree_path.exists(), "worktree directory should exist");

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
    let result = manager.validate_write_paths("task-xyz", &invalid_path).await;
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
