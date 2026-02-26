//! Sprint CC TG.5 — Task genealogy integration tests.

use clawd::tasks::storage::TaskStorage;
use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

async fn make_storage() -> (TaskStorage, TempDir) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = SqlitePoolOptions::new()
        .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    // Run schema migration.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_tasks (
            id TEXT PRIMARY KEY,
            title TEXT,
            status TEXT DEFAULT 'pending',
            type TEXT DEFAULT 'code',
            phase TEXT, \"group\" TEXT, parent_id TEXT,
            severity TEXT DEFAULT 'medium',
            file TEXT, files TEXT DEFAULT '[]', depends_on TEXT DEFAULT '[]',
            tags TEXT DEFAULT '[]', estimated_minutes INTEGER,
            repo_path TEXT DEFAULT '',
            created_at INTEGER, updated_at INTEGER,
            confidence_score REAL, confidence_reasoning TEXT
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS task_genealogy (
            id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(8)))),
            parent_task_id TEXT NOT NULL,
            child_task_id TEXT NOT NULL,
            relationship TEXT NOT NULL DEFAULT 'spawned_from',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE (parent_task_id, child_task_id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    (TaskStorage::new(pool), dir)
}

#[tokio::test]
async fn task_genealogy_parent_child_chain() {
    let (storage, _dir) = make_storage().await;

    // Create parent.
    let parent = storage
        .add_task("parent-1", "Parent Task", None, None, None, None, None, None, None, None, None, None, "")
        .await
        .unwrap();
    assert_eq!(parent.id, "parent-1");

    // Create child.
    let child = storage
        .add_task("child-1", "Child Task", None, None, None, Some("parent-1"), None, None, None, None, None, None, "")
        .await
        .unwrap();
    assert_eq!(child.id, "child-1");

    // Insert genealogy record.
    sqlx::query(
        "INSERT OR IGNORE INTO task_genealogy (parent_task_id, child_task_id, relationship) VALUES (?, ?, ?)",
    )
    .bind("parent-1")
    .bind("child-1")
    .bind("spawned_from")
    .execute(storage.pool())
    .await
    .unwrap();

    // Create grandchild.
    let grandchild = storage
        .add_task("grandchild-1", "Grandchild Task", None, None, None, Some("child-1"), None, None, None, None, None, None, "")
        .await
        .unwrap();
    assert_eq!(grandchild.id, "grandchild-1");

    sqlx::query(
        "INSERT OR IGNORE INTO task_genealogy (parent_task_id, child_task_id, relationship) VALUES (?, ?, ?)",
    )
    .bind("child-1")
    .bind("grandchild-1")
    .bind("spawned_from")
    .execute(storage.pool())
    .await
    .unwrap();

    // Query lineage of child: should have parent as ancestor, grandchild as descendant.
    let ancestors = sqlx::query(
        "SELECT parent_task_id FROM task_genealogy WHERE child_task_id = ?",
    )
    .bind("child-1")
    .fetch_all(storage.pool())
    .await
    .unwrap();

    assert_eq!(ancestors.len(), 1);
    use sqlx::Row;
    assert_eq!(ancestors[0].get::<String, _>("parent_task_id"), "parent-1");

    let descendants = sqlx::query(
        "SELECT child_task_id FROM task_genealogy WHERE parent_task_id = ?",
    )
    .bind("child-1")
    .fetch_all(storage.pool())
    .await
    .unwrap();
    assert_eq!(descendants.len(), 1);
    assert_eq!(descendants[0].get::<String, _>("child_task_id"), "grandchild-1");
}

#[tokio::test]
async fn task_genealogy_duplicate_link_ignored() {
    let (storage, _dir) = make_storage().await;
    storage
        .add_task("p-2", "Parent", None, None, None, None, None, None, None, None, None, None, "")
        .await
        .unwrap();
    storage
        .add_task("c-2", "Child", None, None, None, None, None, None, None, None, None, None, "")
        .await
        .unwrap();

    for _ in 0..3 {
        sqlx::query(
            "INSERT OR IGNORE INTO task_genealogy (parent_task_id, child_task_id, relationship) VALUES (?, ?, ?)",
        )
        .bind("p-2")
        .bind("c-2")
        .bind("spawned_from")
        .execute(storage.pool())
        .await
        .unwrap();
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM task_genealogy WHERE parent_task_id = 'p-2'",
    )
    .fetch_one(storage.pool())
    .await
    .unwrap();
    assert_eq!(count, 1, "duplicate genealogy links should be ignored");
}
