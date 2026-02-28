//! Regenerates `active.md` from DB state.
//! Rules:
//!   - Preserves all headings, free-text, and row order from last parsed version
//!   - Only replaces status symbols in table rows
//!   - Adds new task rows at bottom of their phase table
//!   - Updates "Recently Completed" section with tasks done in last 24h
//!   - Never removes existing rows (deferred tasks stay with 🚫)

use super::storage::AgentTaskRow;
use std::collections::{HashMap, HashSet};

pub fn status_to_symbol(status: &str) -> &'static str {
    match status {
        "done" => "✅",
        "pending" => "🔲",
        "in_progress" => "🚧",
        "blocked" => "❌",
        "deferred" => "🚫",
        "in_qa" => "🟡",
        "in_cr" => "🔍",
        "interrupted" => "⚠️",
        _ => "🔲",
    }
}

/// Given the original active.md content and current DB tasks,
/// returns updated active.md content with only status symbols changed.
/// Tasks in the DB that are absent from the markdown are appended at the
/// bottom so they are never silently lost (M1 fix).
pub fn regenerate(original: &str, db_tasks: &[AgentTaskRow]) -> String {
    // Build lookup: task_id -> current status
    let status_map: HashMap<&str, &str> = db_tasks
        .iter()
        .map(|t| (t.id.as_str(), t.status.as_str()))
        .collect();

    let mut output_lines: Vec<String> = Vec::new();
    let mut seen_ids: HashSet<&str> = HashSet::new();

    for line in original.lines() {
        let trimmed = line.trim();

        // Check if this is a table row that might contain a task
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let cols: Vec<&str> = trimmed
                .split('|')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();

            if cols.len() >= 4 {
                let id_raw = cols[0];
                // Is this a valid task ID?
                let is_task_row = id_raw.len() <= 20
                    && !id_raw.is_empty()
                    && !id_raw.contains("---")
                    && id_raw != "#"
                    && id_raw.to_lowercase() != "id"
                    && id_raw
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_');

                if is_task_row {
                    if let Some(&new_status) = status_map.get(id_raw) {
                        seen_ids.insert(id_raw);
                        let new_symbol = status_to_symbol(new_status);
                        // Replace the last column (status symbol) only
                        let updated = replace_last_table_col(trimmed, new_symbol);
                        output_lines.push(updated);
                        continue;
                    }
                }
            }
        }

        output_lines.push(line.to_string());
    }

    // Append tasks that exist in the DB but were not found in the markdown.
    // This prevents silently dropping tasks created programmatically via RPC.
    let new_tasks: Vec<&AgentTaskRow> = db_tasks
        .iter()
        .filter(|t| !seen_ids.contains(t.id.as_str()))
        .collect();

    if !new_tasks.is_empty() {
        output_lines.push(String::new());
        output_lines.push("## New Tasks (auto-added from database)".to_string());
        output_lines.push(String::new());
        output_lines.push("| ID | Severity | Title | File | Status |".to_string());
        output_lines.push("|---|---|---|---|---|".to_string());
        for task in new_tasks {
            let sym = status_to_symbol(&task.status);
            let severity = task.severity.as_deref().unwrap_or("-");
            let file = task.file.as_deref().unwrap_or("-");
            output_lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                task.id, severity, task.title, file, sym
            ));
        }
    }

    let mut result = output_lines.join("\n");
    // Preserve trailing newline if original had one
    if original.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Replaces the content of the last `|`-delimited cell in a table row.
fn replace_last_table_col(row: &str, new_value: &str) -> String {
    // Find the last `|` before the trailing `|`
    let row = row.trim();
    if let Some(last_pipe) = row.rfind('|') {
        let before_last = &row[..last_pipe];
        if let Some(second_last_pipe) = before_last.rfind('|') {
            let prefix = &row[..second_last_pipe + 1];
            return format!("{} {} |", prefix, new_value);
        }
    }
    row.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: &str, status: &str) -> AgentTaskRow {
        AgentTaskRow {
            id: id.into(),
            status: status.into(),
            title: "Test task".into(),
            task_type: None,
            phase: None,
            group: None,
            parent_id: None,
            severity: Some("medium".into()),
            claimed_by: None,
            claimed_at: None,
            started_at: None,
            completed_at: None,
            last_heartbeat: None,
            file: None,
            files: None,
            depends_on: None,
            blocks: None,
            tags: None,
            notes: None,
            block_reason: None,
            estimated_minutes: None,
            actual_minutes: None,
            repo_path: "/tmp".into(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn replaces_status_symbol() {
        let original = "| FP-C1 | CRITICAL | Fix something | file.dart | 🔲 |";
        let task = AgentTaskRow {
            id: "FP-C1".into(),
            status: "done".into(),
            title: "Fix something".into(),
            task_type: None,
            phase: None,
            group: None,
            parent_id: None,
            severity: Some("critical".into()),
            claimed_by: None,
            claimed_at: None,
            started_at: None,
            completed_at: None,
            last_heartbeat: None,
            file: None,
            files: None,
            depends_on: None,
            blocks: None,
            tags: None,
            notes: None,
            block_reason: None,
            estimated_minutes: None,
            actual_minutes: None,
            repo_path: "/tmp".into(),
            created_at: 0,
            updated_at: 0,
        };
        let result = regenerate(original, &[task]);
        assert!(result.contains("✅"), "Expected ✅ in: {result}");
        assert!(!result.contains("🔲"), "Should not have 🔲 in: {result}");
    }

    #[test]
    fn appends_db_tasks_not_in_markdown() {
        let original = "| FP-C1 | CRITICAL | Fix something | file.dart | 🔲 |\n";
        let in_markdown = make_task("FP-C1", "done");
        let db_only = make_task("FP-C2", "pending");
        let result = regenerate(original, &[in_markdown, db_only]);
        assert!(
            result.contains("FP-C2"),
            "DB-only task should appear in output: {result}"
        );
        assert!(
            result.contains("New Tasks"),
            "Should have auto-added section: {result}"
        );
    }
}
