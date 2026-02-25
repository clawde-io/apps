// SPDX-License-Identifier: MIT
//! Task complexity classifier + split proposal (SI.T09–T10).
//!
//! Classifies an incoming user prompt into one of four complexity tiers and,
//! when the task is complex enough, proposes how it could be split into smaller
//! sub-tasks that are easier for AI agents to handle without context overflow.
//!
//! ## Classification heuristics
//!
//! | Tier           | Prompt length | Signals                                    |
//! |----------------|---------------|--------------------------------------------|
//! | Simple         | < 150 chars   | Single clear question, no code              |
//! | Moderate       | 150–500 chars | One focused task, minimal requirements      |
//! | Complex        | 500–1500 chars| Multi-step, numbered list, code blocks      |
//! | DeepReasoning  | > 1500 chars  | Architecture/design language, multi-file    |
//!
//! These are intentionally coarse — correctness matters less than avoiding
//! unnecessary split proposals for normal prompts.

use serde::Serialize;

// ─── Complexity tiers ─────────────────────────────────────────────────────────

/// Task complexity tier used to decide routing and split proposals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskComplexity {
    /// Single, short question or change.  No split needed.
    Simple,
    /// One focused task.  No split needed.
    Moderate,
    /// Multi-step task.  Split is optional but beneficial for long sessions.
    Complex,
    /// Large design/architecture prompt.  Strong split recommendation.
    DeepReasoning,
}

impl TaskComplexity {
    /// Human-readable label for the Flutter UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Simple => "Simple",
            Self::Moderate => "Moderate",
            Self::Complex => "Complex",
            Self::DeepReasoning => "Deep reasoning",
        }
    }

    /// `true` when a split proposal should be offered.
    pub fn should_split(self) -> bool {
        matches!(self, Self::Complex | Self::DeepReasoning)
    }
}

// ─── Split proposal ───────────────────────────────────────────────────────────

/// A proposed decomposition of a complex task into smaller sub-tasks.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitProposal {
    /// The detected complexity tier.
    pub complexity: TaskComplexity,
    /// Suggested sub-task descriptions (ordered).
    pub subtasks: Vec<String>,
    /// One-line summary of why a split is recommended.
    pub reason: String,
}

// ─── Classifier ──────────────────────────────────────────────────────────────

/// Classify the complexity of a user prompt.
///
/// The classifier is intentionally simple and runs synchronously — it is not an
/// AI call.  False positives (classifying a short prompt as complex) are avoided
/// by requiring multiple signals.
pub fn classify_prompt(prompt: &str) -> TaskComplexity {
    let trimmed = prompt.trim();
    let char_count = trimmed.len();
    let lower = trimmed.to_ascii_lowercase();

    // ── Bonus signals ─────────────────────────────────────────────────────────

    let code_blocks = count_occurrences(trimmed, "```");
    let numbered_steps = count_numbered_list_items(trimmed);
    let design_terms = count_design_terms(&lower);
    let file_refs = count_file_references(&lower);

    // ── Short-prompt early exit — only when no structural signals present ────

    if char_count < 150 && !has_multi_step_signals(&lower) && numbered_steps < 2 {
        return TaskComplexity::Simple;
    }

    // Score based on accumulated signals.
    let complexity_score: usize = (char_count / 300)
        + code_blocks * 2
        + numbered_steps
        + design_terms * 2
        + file_refs;

    if char_count > 1500 || complexity_score >= 6 {
        TaskComplexity::DeepReasoning
    } else if char_count > 500 || complexity_score >= 3 {
        TaskComplexity::Complex
    } else if char_count > 150 || complexity_score >= 1 {
        TaskComplexity::Moderate
    } else {
        TaskComplexity::Simple
    }
}

/// Build a split proposal for prompts classified as Complex or DeepReasoning.
///
/// Returns `None` when the complexity doesn't warrant a split.
pub fn build_split_proposal(prompt: &str) -> Option<SplitProposal> {
    let complexity = classify_prompt(prompt);
    if !complexity.should_split() {
        return None;
    }

    let subtasks = extract_subtasks(prompt);
    let reason = match complexity {
        TaskComplexity::Complex => {
            "This task has multiple steps. Breaking it up will help the AI stay focused \
             and avoid running out of context."
                .to_owned()
        }
        TaskComplexity::DeepReasoning => {
            "This is a large design or architecture task. Splitting it into smaller \
             sessions produces better results and is less likely to hit context limits."
                .to_owned()
        }
        _ => unreachable!(),
    };

    Some(SplitProposal {
        complexity,
        subtasks,
        reason,
    })
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn has_multi_step_signals(lower: &str) -> bool {
    lower.contains("step")
        || lower.contains("first")
        || lower.contains("then")
        || lower.contains("finally")
        || lower.contains("also")
}

fn count_occurrences(text: &str, pattern: &str) -> usize {
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = text[start..].find(pattern) {
        count += 1;
        start += pos + pattern.len();
    }
    count
}

fn count_numbered_list_items(text: &str) -> usize {
    // Count lines that start with "1.", "2.", etc.
    text.lines()
        .filter(|line| {
            let t = line.trim_start();
            t.len() > 2
                && t.as_bytes().first().is_some_and(|b| b.is_ascii_digit())
                && t.as_bytes().get(1).copied() == Some(b'.')
        })
        .count()
}

fn count_design_terms(lower: &str) -> usize {
    let terms = [
        "architecture",
        "refactor",
        "redesign",
        "migration",
        "infrastructure",
        "schema",
        "database",
        "api design",
        "system design",
        "trade-off",
        "tradeoff",
        "implementation plan",
        "multi-repo",
        "monorepo",
    ];
    terms.iter().filter(|t| lower.contains(*t)).count()
}

fn count_file_references(lower: &str) -> usize {
    // Count `.rs`, `.dart`, `.ts`, `.tsx`, `.go` etc. file extension mentions.
    let extensions = [".rs", ".dart", ".ts", ".tsx", ".go", ".py", ".yaml", ".toml", ".json"];
    extensions
        .iter()
        .map(|ext| count_occurrences(lower, ext))
        .sum()
}

/// Extract plausible sub-task descriptions from a prompt.
///
/// Attempts (in order):
///   1. Numbered list items
///   2. Bullet-point items (`-` or `*`)
///   3. Sentences containing imperative verbs
///   4. Fallback: split into chunks by paragraph
fn extract_subtasks(prompt: &str) -> Vec<String> {
    // Try numbered list.
    let numbered: Vec<String> = prompt
        .lines()
        .filter_map(|line| {
            let t = line.trim();
            if t.len() > 3
                && t.as_bytes().first().is_some_and(|b| b.is_ascii_digit())
                && t.as_bytes().get(1).copied() == Some(b'.')
            {
                Some(t[2..].trim().to_owned())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
        .collect();

    if numbered.len() >= 2 {
        return numbered;
    }

    // Try bullet points.
    let bullets: Vec<String> = prompt
        .lines()
        .filter_map(|line| {
            let t = line.trim_start_matches(['-', '*', '•'].as_ref()).trim();
            if line.trim_start().starts_with(['-', '*', '•']) && !t.is_empty() {
                Some(t.to_owned())
            } else {
                None
            }
        })
        .filter(|s| s.len() > 5)
        .collect();

    if bullets.len() >= 2 {
        return bullets;
    }

    // Fallback: split by paragraph.
    prompt
        .split("\n\n")
        .map(|p| p.trim().to_owned())
        .filter(|p| p.len() > 10)
        .take(6)
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_simple() {
        let c = classify_prompt("What is the capital of France?");
        assert_eq!(c, TaskComplexity::Simple);
    }

    #[test]
    fn test_classify_moderate() {
        let c = classify_prompt(
            "Please add a `delete` button to the user profile page. It should show a \
             confirmation modal before sending a DELETE request.",
        );
        assert!(c == TaskComplexity::Moderate || c == TaskComplexity::Simple);
    }

    #[test]
    fn test_classify_complex_numbered() {
        let prompt = "Please do the following:\n\
                      1. Refactor the auth module\n\
                      2. Add unit tests\n\
                      3. Update the README\n\
                      4. Run clippy and fix all warnings\n\
                      5. Open a PR";
        let c = classify_prompt(prompt);
        assert!(c >= TaskComplexity::Complex);
    }

    #[test]
    fn test_classify_deep_reasoning_long() {
        let long = "a".repeat(1600);
        let c = classify_prompt(&long);
        assert_eq!(c, TaskComplexity::DeepReasoning);
    }

    #[test]
    fn test_split_proposal_simple_is_none() {
        let proposal = build_split_proposal("Fix the typo in README.md");
        assert!(proposal.is_none());
    }

    #[test]
    fn test_split_proposal_complex_is_some() {
        let prompt = "Please do the following:\n\
                      1. Design the database schema\n\
                      2. Implement the REST API\n\
                      3. Write integration tests\n\
                      4. Deploy to staging\n\
                      5. Update docs";
        let proposal = build_split_proposal(prompt);
        assert!(proposal.is_some());
        let p = proposal.unwrap();
        assert!(!p.subtasks.is_empty());
    }

    #[test]
    fn test_extract_numbered_subtasks() {
        let prompt = "1. First task\n2. Second task\n3. Third task";
        let tasks = extract_subtasks(prompt);
        assert_eq!(tasks, vec!["First task", "Second task", "Third task"]);
    }
}
