// SPDX-License-Identifier: MIT
//! Heuristic prompt suggestion engine.
//!
//! Generates suggestions from keyword-triggered templates and stored history.
//! All suggestions are deterministic — no AI calls are made here.

use super::model::{PromptSuggestion, SuggestionSource};
use crate::repo_intelligence::RepoProfile;
use anyhow::Result;
use sqlx::SqlitePool;
use tracing::debug;
use uuid::Uuid;

// ─── Template definitions ─────────────────────────────────────────────────────

struct PromptTemplate {
    /// Input prefix that triggers this template.
    prefix: &'static str,
    /// Template text. `{input}` is replaced with the current input.
    template: &'static str,
    /// Label shown to the user.
    context: &'static str,
    /// Base score before history boosting.
    base_score: f32,
}

const TEMPLATES: &[PromptTemplate] = &[
    PromptTemplate {
        prefix: "fix",
        template: "Fix the error in the current file and explain what caused it.",
        context: "Error fix template",
        base_score: 0.85,
    },
    PromptTemplate {
        prefix: "fix",
        template: "Fix all lint and compiler warnings in this repo.",
        context: "Lint fix template",
        base_score: 0.75,
    },
    PromptTemplate {
        prefix: "add",
        template: "Add comprehensive tests for the current module.",
        context: "Test generation template",
        base_score: 0.80,
    },
    PromptTemplate {
        prefix: "add",
        template: "Add inline documentation comments to all public functions.",
        context: "Documentation template",
        base_score: 0.70,
    },
    PromptTemplate {
        prefix: "refactor",
        template: "Refactor this file to improve readability and reduce duplication.",
        context: "Refactor template",
        base_score: 0.78,
    },
    PromptTemplate {
        prefix: "explain",
        template: "Explain how the current module works, including data flow.",
        context: "Explanation template",
        base_score: 0.82,
    },
    PromptTemplate {
        prefix: "review",
        template: "Review this code for correctness, security issues, and performance.",
        context: "Code review template",
        base_score: 0.76,
    },
    PromptTemplate {
        prefix: "optimize",
        template: "Optimize the performance of this function with benchmarks.",
        context: "Performance template",
        base_score: 0.72,
    },
];

// ─── Public interface ─────────────────────────────────────────────────────────

/// Stateless prompt suggestion engine.
pub struct PromptSuggester;

impl PromptSuggester {
    /// Return up to `limit` prompt suggestions for the current input.
    ///
    /// Combines keyword-triggered templates with the top most-used prompts
    /// from the session history.  Repository profile is used to tailor
    /// context labels (e.g. "Rust fix template").
    pub async fn suggest_prompts(
        pool: &SqlitePool,
        current_input: &str,
        session_context: &str,
        repo_profile: &Option<RepoProfile>,
        limit: usize,
    ) -> Result<Vec<PromptSuggestion>> {
        let input_lower = current_input.to_lowercase();
        let mut suggestions: Vec<PromptSuggestion> = Vec::new();

        // ── Template suggestions ──────────────────────────────────────────────
        for tpl in TEMPLATES {
            if input_lower.starts_with(tpl.prefix) || input_lower.is_empty() {
                let lang_label = repo_profile
                    .as_ref()
                    .map(|p| format!(" ({})", p.primary_lang.as_str()))
                    .unwrap_or_default();

                suggestions.push(PromptSuggestion {
                    id: format!("tpl:{}", Uuid::new_v4()),
                    text: tpl.template.to_string(),
                    context: format!("{}{}", tpl.context, lang_label),
                    score: tpl.base_score,
                    source: SuggestionSource::Template,
                });
            }
        }

        // ── Context suggestion when session context is available ──────────────
        if !session_context.is_empty() && !current_input.is_empty() {
            suggestions.push(PromptSuggestion {
                id: format!("ctx:{}", Uuid::new_v4()),
                text: format!("{} in the context of the current session.", current_input),
                context: "Contextual completion".to_string(),
                score: 0.65,
                source: SuggestionSource::Context,
            });
        }

        // ── History suggestions ───────────────────────────────────────────────
        let history = load_top_prompts(pool, 3).await?;
        for (text, use_count) in history {
            // Boost score by use frequency, capped at 0.95.
            let history_score = (0.60 + (use_count as f32 * 0.05)).min(0.95);
            suggestions.push(PromptSuggestion {
                id: format!("hist:{}", Uuid::new_v4()),
                text,
                context: format!("Used {} times", use_count),
                score: history_score,
                source: SuggestionSource::History,
            });
        }

        // Sort by score descending, then deduplicate by text, then truncate.
        suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let mut seen = std::collections::HashSet::new();
        suggestions.retain(|s| seen.insert(s.text.clone()));
        suggestions.truncate(limit);

        debug!(
            count = suggestions.len(),
            input = %current_input,
            "prompt suggestions generated"
        );
        Ok(suggestions)
    }

    /// Record that a prompt was used (increments use count in `prompt_history`).
    ///
    /// Creates the row if it does not exist.
    pub async fn record_prompt_used(
        pool: &SqlitePool,
        prompt: &str,
        session_id: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO prompt_history (prompt_text, session_id, use_count, last_used_at)
            VALUES (?, ?, 1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            ON CONFLICT(prompt_text) DO UPDATE SET
                use_count = use_count + 1,
                last_used_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                session_id = excluded.session_id
            "#,
        )
        .bind(prompt)
        .bind(session_id)
        .execute(pool)
        .await?;
        Ok(())
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Load the top `n` most-used prompts from history.
async fn load_top_prompts(pool: &SqlitePool, n: u32) -> Result<Vec<(String, u32)>> {
    let rows = sqlx::query(
        r#"
        SELECT prompt_text, use_count
        FROM prompt_history
        ORDER BY use_count DESC
        LIMIT ?
        "#,
    )
    .bind(n)
    .fetch_all(pool)
    .await?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .map(|r| {
            let text: String = r.get("prompt_text");
            let count: i64 = r.get("use_count");
            (text, count as u32)
        })
        .collect())
}
