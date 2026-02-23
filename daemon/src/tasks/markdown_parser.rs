/// Parses `active.md` table rows into `ParsedTask` structs.
/// Handles the `| id | sev | title | file | status |` format used in ClawDE active.md files.

#[derive(Debug, Clone)]
pub struct ParsedTask {
    pub id: String,
    pub title: String,
    pub severity: Option<String>,
    pub file: Option<String>,
    pub status: String,
    pub phase: Option<String>,
    pub group: Option<String>,
}

pub fn parse_active_md(content: &str) -> Vec<ParsedTask> {
    let mut tasks = Vec::new();
    let mut current_phase: Option<String> = None;
    let mut current_group: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track headings for phase/group context
        if trimmed.starts_with("### ") {
            current_phase = Some(trimmed.trim_start_matches('#').trim().to_string());
            current_group = None;
            continue;
        }
        if trimmed.starts_with("## ") {
            // Top-level phase heading
            current_phase = Some(trimmed.trim_start_matches('#').trim().to_string());
            current_group = None;
            continue;
        }

        // Table rows: must start and end with `|`
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }

        let cols: Vec<&str> = trimmed
            .split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        // Need at least 4 columns: id, sev, title, status (file optional in some tables)
        if cols.len() < 4 {
            continue;
        }

        // Skip header rows (contain "---" separators or literal "Status" / "#" etc.)
        if cols[0].contains("---") || cols[0] == "#" || cols[0].to_lowercase() == "id" {
            continue;
        }
        // Skip summary/legend rows
        if cols[0].starts_with("Symbol") || cols[0].starts_with("Status") {
            continue;
        }

        let id_raw = cols[0];
        // Task IDs are short alphanumeric codes (e.g. FP-C1, 41a-1, WB-H3)
        if id_raw.len() > 20 || id_raw.contains(' ') {
            continue;
        }
        // Must look like a task ID: letters/digits/hyphens
        if !id_raw.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            continue;
        }

        let id = id_raw.to_string();

        // Last column is status symbol
        let status_col = cols[cols.len() - 1];
        let status = status_symbol_to_str(status_col);

        // If status is unknown, this row might be a data row without status at the end
        if status == "unknown" {
            continue;
        }

        // Second column: severity or task type
        let severity_raw = cols[1].to_uppercase();
        let severity = match severity_raw.as_str() {
            "CRITICAL" => Some("critical".to_string()),
            "HIGH" => Some("high".to_string()),
            "MEDIUM" => Some("medium".to_string()),
            "LOW" => Some("low".to_string()),
            _ => None,
        };

        // Middle columns: title is col[2], file is col[N-2] if 5+ columns
        let title = strip_markdown(cols[2]).to_string();
        let file = if cols.len() >= 5 {
            let f = cols[cols.len() - 2].trim();
            if !f.is_empty() && f != "—" && f != "-" && f != "N/A" {
                Some(f.to_string())
            } else {
                None
            }
        } else {
            None
        };

        // Derive group from id prefix (e.g. "FP" from "FP-C1", "41a" from "41a-1")
        let group = id.split('-').next().map(|s| s.to_string());
        if let Some(ref g) = group {
            if current_group.as_deref() != Some(g.as_str()) {
                current_group = Some(g.clone());
            }
        }

        tasks.push(ParsedTask {
            id,
            title,
            severity,
            file,
            status: status.to_string(),
            phase: current_phase.clone(),
            group: current_group.clone(),
        });
    }

    tasks
}

fn status_symbol_to_str(s: &str) -> &'static str {
    match s {
        "✅" => "done",
        "🔲" => "pending",
        "🚧" => "in_progress",
        "❌" => "blocked",
        "🚫" => "deferred",
        "🟡" => "in_qa",
        "🔍" => "in_cr",
        "⚠️" => "interrupted",
        _ => "unknown",
    }
}

fn strip_markdown(s: &str) -> &str {
    // Remove backtick code spans, bold/italic markers
    let s = s.trim();
    // Simple: just trim, the heavy markdown processing can be added later
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_task_row() {
        let md = r#"
### QA-FIX GROUP 2

| # | Sev | Task | File | Status |
|---|-----|------|------|--------|
| FP-C1 | CRITICAL | Fix `RepoStatus.fromJson` | clawd_proto/lib/src/repo_status.dart | ✅ |
| FP-C2 | HIGH | Fix FileState enum | clawd_proto/lib/src/repo_status.dart | 🔲 |
"#;
        let tasks = parse_active_md(md);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "FP-C1");
        assert_eq!(tasks[0].status, "done");
        assert_eq!(tasks[0].severity.as_deref(), Some("critical"));
        assert_eq!(tasks[1].id, "FP-C2");
        assert_eq!(tasks[1].status, "pending");
        assert_eq!(tasks[0].phase.as_deref(), Some("QA-FIX GROUP 2"));
    }

    #[test]
    fn skips_header_and_separator_rows() {
        let md = "| # | Sev | Task | File | Status |\n|---|-----|------|------|--------|\n";
        let tasks = parse_active_md(md);
        assert!(tasks.is_empty());
    }
}
