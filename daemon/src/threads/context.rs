//! Context isolation for task threads (Phase 43f).
//!
//! Task threads must NOT inherit the full control thread conversation history.
//! This module builds the minimal context they need: task spec + relevant
//! file snapshots + project coding rules.
//!
//! This keeps task threads focused, avoids context pollution between concurrent
//! tasks, and prevents the control thread's planning discussion from being
//! mistaken for implementation instructions.

/// Build the initial OpenAI-compatible messages array for a task thread.
///
/// Returns a `vec` of message objects suitable for passing directly to any
/// OpenAI-compatible provider (Claude, Codex, etc.).
///
/// # Arguments
///
/// * `task_spec`       – The task's JSON spec (title, summary, acceptance_criteria, …)
/// * `relevant_files`  – `(path, content)` pairs — only files relevant to the task
/// * `coding_rules`    – Project coding standards (from `.claude/rules/` or equivalent)
pub fn build_task_context(
    task_spec: &serde_json::Value,
    relevant_files: &[(String, String)],
    coding_rules: &str,
) -> Vec<serde_json::Value> {
    let title = task_spec
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Unnamed task");

    let summary = task_spec
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let acceptance = task_spec
        .get("acceptance_criteria")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("- {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    let test_plan = task_spec
        .get("test_plan")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let risk = task_spec
        .get("risk_level")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");

    // ── System prompt ────────────────────────────────────────────────────────
    let mut system = format!(
        "You are a task-scoped AI agent. Your ONLY job is to complete the task below.\n\
         You are working in an isolated git worktree — your changes will not affect other work.\n\
         Risk level for this task: {risk}.\n\n\
         ## Task: {title}\n"
    );

    if !summary.is_empty() {
        system.push_str(&format!("\n{summary}\n"));
    }
    if !acceptance.is_empty() {
        system.push_str(&format!("\n## Acceptance Criteria\n{acceptance}\n"));
    }
    if !test_plan.is_empty() {
        system.push_str(&format!("\n## Test Plan\n{test_plan}\n"));
    }
    if !coding_rules.is_empty() {
        system.push_str(&format!("\n## Coding Rules\n{coding_rules}\n"));
    }

    let mut messages = vec![serde_json::json!({ "role": "system", "content": system })];

    // ── Relevant file snapshots ──────────────────────────────────────────────
    if !relevant_files.is_empty() {
        let mut file_block = "## Relevant Files\n\n".to_string();
        for (path, content) in relevant_files {
            // Truncate very large files to avoid blowing the context window.
            const MAX_FILE_CHARS: usize = 8_000;
            let truncated = if content.len() > MAX_FILE_CHARS {
                format!(
                    "{}\n\n[... truncated — {} chars omitted ...]",
                    &content[..MAX_FILE_CHARS],
                    content.len() - MAX_FILE_CHARS
                )
            } else {
                content.clone()
            };
            file_block.push_str(&format!("### `{path}`\n```\n{truncated}\n```\n\n"));
        }
        messages.push(serde_json::json!({
            "role": "user",
            "content": file_block,
        }));
    }

    messages
}
