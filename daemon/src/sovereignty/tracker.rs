//! Sprint DD TS.1 — AI Tool Sovereignty Tracker.
//!
//! Watches for writes by other AI tools (Copilot, Cursor, Continue, Codex)
//! to the project directory and logs them as sovereignty events.

use anyhow::Result;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Known AI tool signatures by directory/file pattern.
const TOOL_PATTERNS: &[(&str, &str)] = &[
    (".copilot", "copilot"),
    (".cursor", "cursor"),
    (".continue", "continue"),
    (".codex", "codex"),
    (".aider", "aider"),
    (".tabnine", "tabnine"),
    (".codeium", "codeium"),
];

/// Detect which AI tool (if any) corresponds to a changed path.
pub fn detect_tool(path: &str) -> Option<&'static str> {
    for (pattern, tool_id) in TOOL_PATTERNS {
        if path.contains(pattern) {
            return Some(tool_id);
        }
    }
    None
}

/// Record a sovereignty event when another AI tool touches the project.
pub async fn record_sovereignty_event(
    pool: &SqlitePool,
    tool_id: &str,
    event_type: &str,
    file_paths: &[String],
    active_session_id: Option<&str>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let files_json = serde_json::to_string(file_paths)?;

    sqlx::query(
        "INSERT INTO sovereignty_events (id, tool_id, event_type, file_paths, session_active_at_detection)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(tool_id)
    .bind(event_type)
    .bind(&files_json)
    .bind(active_session_id)
    .execute(pool)
    .await?;

    Ok(id)
}

/// A sovereignty event row returned by queries.
#[derive(Debug, Clone)]
pub struct SovereigntyEvent {
    pub id: String,
    pub tool_id: String,
    pub event_type: String,
    pub file_paths: Vec<String>,
    pub detected_at: String,
}

/// Per-tool summary for `sovereignty.report`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolSummary {
    pub tool_id: String,
    pub event_count: i64,
    pub files_touched: Vec<String>,
    pub last_seen: String,
}

/// Fetch the 7-day sovereignty report.
pub async fn get_report(pool: &SqlitePool) -> Result<Vec<ToolSummary>> {
    use sqlx::Row as _;

    let rows = sqlx::query(
        "SELECT tool_id,
                COUNT(*) AS event_count,
                GROUP_CONCAT(file_paths, '|||') AS paths_blob,
                MAX(detected_at) AS last_seen
         FROM sovereignty_events
         WHERE detected_at >= datetime('now', '-7 days')
         GROUP BY tool_id
         ORDER BY event_count DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut summaries = Vec::new();
    for row in rows {
        let tool_id: String = row.get("tool_id");
        let event_count: i64 = row.get("event_count");
        let last_seen: String = row.get("last_seen");
        let paths_blob: String = row.get::<Option<String>, _>("paths_blob").unwrap_or_default();

        // Collect unique file paths across all events for this tool.
        let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();
        for chunk in paths_blob.split("|||") {
            if let Ok(paths) = serde_json::from_str::<Vec<String>>(chunk) {
                files.extend(paths);
            }
        }

        summaries.push(ToolSummary {
            tool_id,
            event_count,
            files_touched: files.into_iter().collect(),
            last_seen,
        });
    }

    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_copilot() {
        assert_eq!(detect_tool("/project/.copilot/config.json"), Some("copilot"));
    }

    #[test]
    fn test_detect_cursor() {
        assert_eq!(detect_tool("/project/.cursor/settings.json"), Some("cursor"));
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_tool("/project/src/main.rs"), None);
    }

    #[test]
    fn test_detect_aider() {
        assert_eq!(detect_tool("/home/user/.aider/cache"), Some("aider"));
    }
}
