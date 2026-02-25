// SPDX-License-Identifier: MIT
//! Provider intent router — pure heuristic, no subprocess calls, no ML.
//!
//! Classifies an incoming message to determine whether Claude Code or Codex
//! is the better provider for the task.  Used by `session.create` when the
//! caller passes `provider: "auto"`.
//!
//! Signal scoring:
//!   Codex signals:  "debug", "explain", "review", "why", "error", "bug",
//!                   "what does", "what is"
//!   Claude signals: "generate", "refactor", "implement", "build", "create",
//!                   "write", "add", "fix"
//!
//!   If codex_score > claude_score → Codex
//!   Otherwise (including tie) → Claude (safe default)

/// The provider selected by the auto-router.
#[derive(Debug, Clone, PartialEq)]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    /// Return the provider name as used in the daemon protocol (matches Dart ProviderType.name).
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        }
    }
}

/// Classify an optional initial message to choose the best provider.
///
/// `_repo_languages` is accepted for future use (language-based routing)
/// but is not yet used in the scoring logic.
///
/// The routing decision is **logged at `debug` level only** to avoid
/// leaking user message content into info-level logs.
pub fn classify_intent(
    initial_message: Option<&str>,
    _repo_languages: &[String],
) -> Provider {
    let msg = initial_message.unwrap_or("").to_lowercase();

    let codex_signals: &[&str] = &[
        "debug", "explain", "review", "why", "error", "bug",
        "what does", "what is",
    ];
    let claude_signals: &[&str] = &[
        "generate", "refactor", "implement", "build", "create",
        "write", "add", "fix",
    ];

    let codex_score: usize = codex_signals.iter().filter(|&&s| msg.contains(s)).count();
    let claude_score: usize = claude_signals.iter().filter(|&&s| msg.contains(s)).count();

    tracing::debug!(
        codex_score = codex_score,
        claude_score = claude_score,
        "provider auto-routing decision"
    );

    if codex_score > claude_score {
        Provider::Codex
    } else {
        Provider::Claude
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn no_langs() -> Vec<String> {
        vec![]
    }

    #[test]
    fn test_route_debug_intent_to_codex() {
        let p = classify_intent(Some("debug this error in my function"), &no_langs());
        assert_eq!(p, Provider::Codex);
    }

    #[test]
    fn test_route_explain_intent_to_codex() {
        let p = classify_intent(Some("explain what this code does"), &no_langs());
        assert_eq!(p, Provider::Codex);
    }

    #[test]
    fn test_route_generate_intent_to_claude() {
        let p = classify_intent(Some("implement a login page with JWT"), &no_langs());
        assert_eq!(p, Provider::Claude);
    }

    #[test]
    fn test_route_refactor_intent_to_claude() {
        let p = classify_intent(Some("refactor this module to use async/await"), &no_langs());
        assert_eq!(p, Provider::Claude);
    }

    #[test]
    fn test_route_ambiguous_defaults_to_claude() {
        // "fix" (claude signal) ties with "bug" (codex signal) → claude (safe default)
        let p = classify_intent(Some("fix this bug"), &no_langs());
        // codex_score=1 (bug), claude_score=1 (fix) → tie → Claude
        assert_eq!(p, Provider::Claude);
    }

    #[test]
    fn test_route_empty_message_defaults_to_claude() {
        let p = classify_intent(None, &no_langs());
        assert_eq!(p, Provider::Claude);
    }

    #[test]
    fn test_route_empty_string_defaults_to_claude() {
        let p = classify_intent(Some(""), &no_langs());
        assert_eq!(p, Provider::Claude);
    }

    #[test]
    fn test_route_review_to_codex() {
        let p = classify_intent(Some("do a code review of this PR"), &no_langs());
        assert_eq!(p, Provider::Codex);
    }

    #[test]
    fn test_provider_as_str() {
        assert_eq!(Provider::Claude.as_str(), "claude");
        assert_eq!(Provider::Codex.as_str(), "codex");
    }
}
