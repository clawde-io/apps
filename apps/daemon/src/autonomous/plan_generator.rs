// SPDX-License-Identifier: MIT
//! Plan generator — AE.T01/T03 (Autonomous Execution Engine, Sprint J).
//!
//! Before dispatching a user message, the orchestration turn generates a
//! JIRA-style `AePlan` using a purely heuristic approach (no AI call).
//!
//! ## Extraction rules
//! - **title**: first noun phrase — the first sentence up to 60 chars, or the
//!   first line of the message.
//! - **requirements**: lines starting with `-` / `*` / `•`, or sentences
//!   containing the words "must", "should", "need", or "require".
//! - **files_expected**: file names/paths mentioned inside ``` code fences or
//!   bare tokens ending with a recognised source extension.
//! - **definition_of_done**: lines containing "done when", "complete when",
//!   "success when", "ac:", or "acceptance criteria".
//! - **ai_instructions**: a compact reminder block assembled from the above.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// ─── AePlan ───────────────────────────────────────────────────────────────────

/// A JIRA-style task specification generated before executing a user message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AePlan {
    pub id: String,
    pub session_id: String,
    pub title: String,
    /// Individual requirement strings (what the AI must do).
    pub requirements: Vec<String>,
    /// Definition-of-done criteria.
    pub definition_of_done: Vec<String>,
    /// Files the AI is expected to create or modify.
    pub files_expected: Vec<PathBuf>,
    /// Instruction block injected into the system prompt context.
    pub ai_instructions: String,
    pub created_at: String,
    /// Set when the user taps "Approve Plan" in the UI.
    pub approved_at: Option<String>,
    /// Optional parent plan ID (AE.T14 — task genealogy).
    pub parent_task_id: Option<String>,
}

impl AePlan {
    /// Build the `ai_instructions` field from the extracted plan fields.
    pub fn build_instructions(&mut self) {
        let mut parts = Vec::new();
        parts.push(format!("Task: {}", self.title));
        if !self.requirements.is_empty() {
            parts.push(format!(
                "Requirements:\n{}",
                self.requirements
                    .iter()
                    .map(|r| format!("- {r}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        if !self.definition_of_done.is_empty() {
            parts.push(format!(
                "Definition of done:\n{}",
                self.definition_of_done
                    .iter()
                    .map(|d| format!("- {d}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        if !self.files_expected.is_empty() {
            let paths: Vec<String> = self
                .files_expected
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            parts.push(format!("Expected files:\n{}", paths.join(", ")));
        }
        self.ai_instructions = parts.join("\n\n");
    }
}

// ─── PlanGenerator ────────────────────────────────────────────────────────────

/// Heuristic plan extractor — no AI call, runs synchronously.
pub struct PlanGenerator;

impl PlanGenerator {
    /// Generate an `AePlan` from a raw user message.
    ///
    /// The `session_id` is recorded in the plan for storage and push events.
    pub fn generate_plan(message: &str, session_id: &str) -> Result<AePlan> {
        let title = extract_title(message);
        let requirements = extract_requirements(message);
        let definition_of_done = extract_dod(message);
        let files_expected = extract_files(message);

        let mut plan = AePlan {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_owned(),
            title,
            requirements,
            definition_of_done,
            files_expected,
            ai_instructions: String::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            approved_at: None,
            parent_task_id: None,
        };
        plan.build_instructions();
        Ok(plan)
    }
}

// ─── Extraction helpers ───────────────────────────────────────────────────────

/// Extract a short title from the first meaningful line or sentence.
fn extract_title(message: &str) -> String {
    // Try the first non-empty line first.
    let first_line = message
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("Task");

    // If the first line is a bullet / header, strip the decoration.
    let stripped = first_line
        .trim_start_matches(['#', '-', '*', '•'].as_ref())
        .trim();

    // Truncate to first sentence boundary or 80 chars.
    let truncated = if let Some(pos) = stripped.find(['.', '?', '!']) {
        &stripped[..=pos]
    } else {
        stripped
    };

    let title = if truncated.len() > 80 {
        format!("{}…", &truncated[..77])
    } else {
        truncated.to_owned()
    };

    if title.is_empty() {
        "Task".to_owned()
    } else {
        title
    }
}

/// Extract requirement strings from the message.
///
/// Collects:
/// - Lines starting with a list marker (`-`, `*`, `•`)
/// - Lines/sentences containing modal verbs (`must`, `should`, `need`, `require`)
fn extract_requirements(message: &str) -> Vec<String> {
    let mut reqs: Vec<String> = Vec::new();

    for line in message.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let is_bullet = trimmed.starts_with(['-', '*', '•'].as_ref());
        if is_bullet {
            let content = trimmed
                .trim_start_matches(['-', '*', '•'].as_ref())
                .trim()
                .to_owned();
            if !content.is_empty() && content.len() > 3 {
                reqs.push(content);
                continue;
            }
        }

        let lower = trimmed.to_ascii_lowercase();
        let has_modal = lower.contains(" must ")
            || lower.contains(" should ")
            || lower.contains(" need ")
            || lower.contains(" require ")
            || lower.starts_with("must ")
            || lower.starts_with("should ")
            || lower.starts_with("need ")
            || lower.starts_with("require ");

        if has_modal && trimmed.len() > 10 {
            reqs.push(trimmed.to_owned());
        }
    }

    // Deduplicate while preserving order.
    let mut seen = std::collections::HashSet::new();
    reqs.retain(|r| seen.insert(r.clone()));
    reqs
}

/// Extract definition-of-done items.
///
/// Looks for lines containing "done when", "complete when", "success when",
/// "ac:", "acceptance criteria", or "definition of done".
fn extract_dod(message: &str) -> Vec<String> {
    let mut dod: Vec<String> = Vec::new();
    let mut in_dod_section = false;

    for line in message.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        // Section header detection — subsequent bullet lines belong to DoD.
        if lower.contains("acceptance criteria")
            || lower.contains("definition of done")
            || lower.contains("done when")
            || lower.contains("complete when")
            || lower.starts_with("ac:")
        {
            in_dod_section = true;
            // If the line itself has content beyond the header, capture it.
            let after_colon = if let Some(pos) = trimmed.find(':') {
                trimmed[pos + 1..].trim()
            } else {
                ""
            };
            if !after_colon.is_empty() {
                dod.push(after_colon.to_owned());
            }
            continue;
        }

        if in_dod_section {
            // Blank line ends the section.
            if trimmed.is_empty() {
                in_dod_section = false;
                continue;
            }
            let is_bullet = trimmed.starts_with(['-', '*', '•'].as_ref())
                || (trimmed.len() > 2
                    && trimmed.as_bytes()[0].is_ascii_digit()
                    && trimmed.as_bytes()[1] == b'.');
            if is_bullet {
                let content = trimmed
                    .trim_start_matches(['-', '*', '•'].as_ref())
                    .trim()
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .trim_start_matches('.')
                    .trim()
                    .to_owned();
                if !content.is_empty() {
                    dod.push(content);
                }
            }
        }
    }

    dod
}

/// Recognised source file extensions for path extraction.
const SOURCE_EXTENSIONS: &[&str] = &[
    ".rs", ".dart", ".ts", ".tsx", ".js", ".go", ".py", ".rb", ".java", ".kt", ".swift", ".c",
    ".cpp", ".h", ".toml", ".yaml", ".yml", ".json", ".sql", ".md",
];

/// Extract file paths mentioned in the message.
///
/// Scans code fences for filenames and also looks for bare path-like tokens
/// ending with a recognised source extension.
fn extract_files(message: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut in_fence = false;
    let mut fence_lang: Option<String> = None;

    for line in message.lines() {
        let trimmed = line.trim();

        // Toggle code fence state.
        if trimmed.starts_with("```") {
            if in_fence {
                in_fence = false;
                fence_lang = None;
            } else {
                in_fence = true;
                let lang = trimmed.trim_start_matches('`').trim();
                fence_lang = if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_owned())
                };
            }
            continue;
        }

        // Inside a fence, look for comment-style filenames like `// path/to/file.rs`
        // or `# path/to/file.py`.
        if in_fence {
            let _ = fence_lang.as_deref();
            let content = trimmed
                .trim_start_matches("//")
                .trim_start_matches('#')
                .trim_start_matches("--")
                .trim();
            if looks_like_path(content) {
                files.push(PathBuf::from(content));
            }
            continue;
        }

        // Outside fences, tokenise and look for path-like tokens.
        for token in trimmed.split_whitespace() {
            // Two-pass strip: quotes/brackets, then trailing punctuation, then
            // quotes/brackets again (handles patterns like `path/file.rs`.)
            let s1 = token.trim_matches(['`', '"', '\'', '(', ')'].as_ref());
            let s2 = s1.trim_end_matches(['.', ',', ':', ';'].as_ref());
            let clean = s2.trim_end_matches(['`', '"', '\''].as_ref());
            if looks_like_path(clean) {
                files.push(PathBuf::from(clean));
            }
        }
    }

    // Deduplicate.
    files.sort();
    files.dedup();
    files
}

/// Returns `true` if `token` looks like a source file path.
fn looks_like_path(token: &str) -> bool {
    if token.len() < 4 {
        return false;
    }
    SOURCE_EXTENSIONS.iter().any(|ext| token.ends_with(ext))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title_first_line() {
        let msg = "Add a login button to the header.\nIt should redirect to /login.";
        let title = extract_title(msg);
        assert_eq!(title, "Add a login button to the header.");
    }

    #[test]
    fn test_extract_title_strips_bullet() {
        let msg = "- Implement the OAuth flow";
        assert_eq!(extract_title(msg), "Implement the OAuth flow");
    }

    #[test]
    fn test_extract_title_truncates_long() {
        let long = "a".repeat(100);
        let title = extract_title(&long);
        assert!(title.len() <= 80);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn test_extract_requirements_bullets() {
        let msg = "Please implement the following:\n- Add a login button\n- Validate the form\n- Submit to /api/auth";
        let reqs = extract_requirements(msg);
        assert!(reqs.contains(&"Add a login button".to_owned()));
        assert!(reqs.contains(&"Validate the form".to_owned()));
        assert!(reqs.contains(&"Submit to /api/auth".to_owned()));
    }

    #[test]
    fn test_extract_requirements_modal() {
        let msg =
            "The endpoint must return 401 on invalid token. The handler should log the error.";
        let reqs = extract_requirements(msg);
        assert!(!reqs.is_empty());
        assert!(reqs.iter().any(|r| r.contains("must return")));
    }

    #[test]
    fn test_extract_files_from_fence() {
        let msg = "```rust\n// src/main.rs\nfn main() {}\n```";
        let files = extract_files(msg);
        assert!(files.contains(&PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_extract_files_bare_token() {
        let msg = "Please update `daemon/src/lib.rs` and `desktop/lib/app.dart`.";
        let files = extract_files(msg);
        assert!(files.contains(&PathBuf::from("daemon/src/lib.rs")));
        assert!(files.contains(&PathBuf::from("desktop/lib/app.dart")));
    }

    #[test]
    fn test_extract_dod() {
        let msg = "Acceptance criteria:\n- All tests pass\n- Clippy is clean\n- No TODOs in code";
        let dod = extract_dod(msg);
        assert!(dod.contains(&"All tests pass".to_owned()));
        assert!(dod.contains(&"Clippy is clean".to_owned()));
    }

    #[test]
    fn test_generate_plan_roundtrip() {
        let msg = "Add authentication middleware.\n\
            - Must validate JWT tokens\n\
            - Should return 401 on failure\n\
            \nAcceptance criteria:\n- Returns 200 on valid token";
        let plan = PlanGenerator::generate_plan(msg, "sess-001").expect("plan generation failed");
        assert!(!plan.id.is_empty());
        assert_eq!(plan.session_id, "sess-001");
        assert!(!plan.title.is_empty());
        assert!(!plan.ai_instructions.is_empty());
    }
}
