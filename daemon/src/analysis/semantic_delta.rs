//! Sprint DD PP.1 — Semantic delta engine.
//!
//! Classifies git diffs into semantic change types using keyword analysis.
//! No live AI call — uses pattern matching on diff hunks and commit messages.
//!
//! ## Semantic change types
//!
//! | Type | Detection |
//! | ---- | --------- |
//! | `feature_added` | new function/struct/module exported; commit message "add", "feat", "new" |
//! | `bug_fixed` | commit message "fix", "bug", "crash", "error"; diff removes error handling |
//! | `refactored` | same API surface but internals changed; commit message "refactor", "cleanup" |
//! | `test_added` | `*_test.rs`, `*_spec.dart`, `*.test.ts` in diff; commit message "test" |
//! | `config_changed` | `.toml`, `.yaml`, `.json`, `.env` files changed |
//! | `dependency_updated` | `Cargo.toml`, `pubspec.yaml`, `package.json` changed |

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// A classified semantic change event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticEvent {
    pub id: String,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub event_type: SemanticEventType,
    pub affected_files: Vec<String>,
    pub summary_text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticEventType {
    FeatureAdded,
    BugFixed,
    Refactored,
    TestAdded,
    ConfigChanged,
    DependencyUpdated,
}

impl SemanticEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FeatureAdded => "feature_added",
            Self::BugFixed => "bug_fixed",
            Self::Refactored => "refactored",
            Self::TestAdded => "test_added",
            Self::ConfigChanged => "config_changed",
            Self::DependencyUpdated => "dependency_updated",
        }
    }
}

/// Classify a set of changed files + commit message into a semantic event type.
pub fn classify_change(changed_files: &[String], commit_msg: &str) -> SemanticEventType {
    let msg_lower = commit_msg.to_lowercase();

    // Dependency files take priority.
    if changed_files.iter().any(|f| {
        f.ends_with("Cargo.toml")
            || f.ends_with("pubspec.yaml")
            || f.ends_with("package.json")
            || f.ends_with("Cargo.lock")
    }) {
        return SemanticEventType::DependencyUpdated;
    }

    // Config files.
    if changed_files.iter().all(|f| {
        f.ends_with(".toml")
            || f.ends_with(".yaml")
            || f.ends_with(".yml")
            || f.ends_with(".json")
            || f.ends_with(".env")
    }) {
        return SemanticEventType::ConfigChanged;
    }

    // Test files.
    if changed_files.iter().any(|f| {
        f.contains("_test.") || f.contains("_spec.") || f.contains(".test.") || f.contains("tests/")
    }) || msg_lower.contains("test")
        || msg_lower.contains("spec")
    {
        return SemanticEventType::TestAdded;
    }

    // Bug fix.
    if msg_lower.contains("fix")
        || msg_lower.contains("bug")
        || msg_lower.contains("crash")
        || msg_lower.contains("error")
        || msg_lower.contains("revert")
    {
        return SemanticEventType::BugFixed;
    }

    // Refactoring.
    if msg_lower.contains("refactor")
        || msg_lower.contains("cleanup")
        || msg_lower.contains("reorganize")
        || msg_lower.contains("rename")
        || msg_lower.contains("move")
    {
        return SemanticEventType::Refactored;
    }

    // Default: feature added.
    SemanticEventType::FeatureAdded
}

/// Persist a semantic event to the database.
pub async fn record_event(
    pool: &SqlitePool,
    session_id: Option<&str>,
    task_id: Option<&str>,
    event_type: SemanticEventType,
    affected_files: &[String],
    summary_text: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let files_json = serde_json::to_string(affected_files)?;

    sqlx::query(
        "INSERT INTO semantic_events (id, session_id, task_id, event_type, affected_files, summary_text)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(session_id)
    .bind(task_id)
    .bind(event_type.as_str())
    .bind(&files_json)
    .bind(summary_text)
    .execute(pool)
    .await?;

    Ok(id)
}

/// Fetch semantic events for the project pulse RPC.
pub async fn get_pulse(pool: &SqlitePool, days: i64) -> Result<Vec<SemanticEvent>> {
    use sqlx::Row as _;

    let rows = sqlx::query(
        "SELECT id, session_id, task_id, event_type, affected_files, summary_text, created_at
         FROM semantic_events
         WHERE created_at >= datetime('now', ? || ' days')
         ORDER BY created_at DESC",
    )
    .bind(format!("-{}", days))
    .fetch_all(pool)
    .await?;

    let mut events = Vec::new();
    for row in rows {
        let event_type_str: String = row.get("event_type");
        let event_type = match event_type_str.as_str() {
            "feature_added" => SemanticEventType::FeatureAdded,
            "bug_fixed" => SemanticEventType::BugFixed,
            "refactored" => SemanticEventType::Refactored,
            "test_added" => SemanticEventType::TestAdded,
            "config_changed" => SemanticEventType::ConfigChanged,
            _ => SemanticEventType::DependencyUpdated,
        };

        let files_str: String = row.get("affected_files");
        let affected_files: Vec<String> = serde_json::from_str(&files_str).unwrap_or_default();

        events.push(SemanticEvent {
            id: row.get("id"),
            session_id: row.get("session_id"),
            task_id: row.get("task_id"),
            event_type,
            affected_files,
            summary_text: row.get("summary_text"),
            created_at: row.get("created_at"),
        });
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_dependency() {
        let files = vec!["Cargo.toml".to_string()];
        assert_eq!(
            classify_change(&files, "update deps"),
            SemanticEventType::DependencyUpdated
        );
    }

    #[test]
    fn test_classify_bug_fix() {
        let files = vec!["src/auth.rs".to_string()];
        assert_eq!(
            classify_change(&files, "fix: token refresh crash"),
            SemanticEventType::BugFixed
        );
    }

    #[test]
    fn test_classify_test_added() {
        let files = vec!["tests/auth_test.rs".to_string()];
        assert_eq!(
            classify_change(&files, "add auth tests"),
            SemanticEventType::TestAdded
        );
    }

    #[test]
    fn test_classify_feature() {
        let files = vec!["src/features/export.rs".to_string()];
        assert_eq!(
            classify_change(&files, "add session export"),
            SemanticEventType::FeatureAdded
        );
    }

    #[test]
    fn test_classify_refactor() {
        let files = vec!["src/ipc/mod.rs".to_string()];
        assert_eq!(
            classify_change(&files, "refactor: extract handler"),
            SemanticEventType::Refactored
        );
    }
}
