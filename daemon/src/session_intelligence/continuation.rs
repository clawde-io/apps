// SPDX-License-Identifier: MIT
//! Auto-continuation + premature stop detection (SI.T11–T12).
//!
//! ## Premature stop detection (SI.T12)
//!
//! Some AI responses end mid-thought because:
//!   * The model hit its per-response output token limit.
//!   * The model produced a self-interrupting phrase like "I'll continue…".
//!   * The response was literally truncated by the CLI layer.
//!
//! This module classifies the stop reason so the session layer can decide
//! whether to automatically send a continuation prompt.
//!
//! ## Auto-continuation (SI.T11)
//!
//! When the stop reason suggests the model was cut off rather than finished,
//! `should_auto_continue` returns `true`.  The caller is responsible for
//! sending the continuation prompt (e.g. `"Please continue."`) and for
//! limiting the total number of auto-continues per turn to avoid loops.

/// Reason an AI turn stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The model finished normally — the task is complete.
    Complete,
    /// The response ended mid-sentence or mid-paragraph — likely hit token limit.
    Truncated,
    /// The model explicitly signalled that it will continue in a follow-up.
    SelfInterrupted,
    /// The context window is full — compression should happen before continuing.
    ContextFull,
    /// The AI provider returned a rate-limit error.
    RateLimited,
    /// Could not determine the stop reason.
    Unknown,
}

impl StopReason {
    /// Human-readable label for logging and the Flutter UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Truncated => "truncated",
            Self::SelfInterrupted => "self_interrupted",
            Self::ContextFull => "context_full",
            Self::RateLimited => "rate_limited",
            Self::Unknown => "unknown",
        }
    }
}

/// `true` if the session layer should automatically send a continuation prompt.
///
/// Auto-continuation is safe for `Truncated` and `SelfInterrupted`.  It is
/// never triggered for `Complete` (done), `ContextFull` (need compression first),
/// or `RateLimited` (need to back off).
pub fn should_auto_continue(reason: StopReason) -> bool {
    matches!(reason, StopReason::Truncated | StopReason::SelfInterrupted)
}

/// Analyse an assistant response to determine why the turn stopped.
///
/// `last_event_type` is the final JSON-RPC event emitted by the runner
/// (e.g. `"result"` with `subtype: "success"` vs. `"error_during_exec"`).
/// Pass `None` if unavailable.
///
/// The classifier uses cheap string heuristics — no AI calls.
pub fn detect_stop_reason(response: &str, last_event_type: Option<&str>) -> StopReason {
    // ── Check explicit event type first ──────────────────────────────────────
    if let Some(event) = last_event_type {
        if event.contains("rate_limit") || event.contains("rate-limit") {
            return StopReason::RateLimited;
        }
        if event.contains("context_window") || event.contains("context_full") {
            return StopReason::ContextFull;
        }
    }

    let trimmed = response.trim();

    // Empty response = unknown (could be network error).
    if trimmed.is_empty() {
        return StopReason::Unknown;
    }

    // ── Self-interruption patterns ────────────────────────────────────────────
    if has_self_interruption(trimmed) {
        return StopReason::SelfInterrupted;
    }

    // ── Truncation heuristics ─────────────────────────────────────────────────
    if looks_truncated(trimmed) {
        return StopReason::Truncated;
    }

    // ── Completion signals ────────────────────────────────────────────────────
    if looks_complete(trimmed) {
        return StopReason::Complete;
    }

    StopReason::Unknown
}

// ─── Heuristic helpers ────────────────────────────────────────────────────────

/// Phrases the model uses when it knows it was interrupted and will continue.
fn has_self_interruption(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let patterns = [
        "i'll continue",
        "i will continue",
        "continuing in the next",
        "to be continued",
        "continued below",
        "continued in part",
        "see the next message",
        "part 1 of",
        "(part 1",
        "[continues]",
        "[to be continued]",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

/// Signals that the response was cut off rather than finished naturally.
fn looks_truncated(text: &str) -> bool {
    // Ends without sentence-ending punctuation and is not a code block.
    let last_char = text.chars().last().unwrap_or(' ');
    let no_terminal_punct = !matches!(last_char, '.' | '!' | '?' | ')' | '}' | '`' | '"' | '\'');

    // Open code fences (odd number of ```) are a strong truncation signal.
    let open_code_fence = text.matches("```").count() % 2 == 1;

    // Ends with a conjunction or mid-clause indicator.
    let mid_clause_end = {
        let lower = text.to_ascii_lowercase();
        lower.ends_with(" and")
            || lower.ends_with(" or")
            || lower.ends_with(" but")
            || lower.ends_with(" the")
            || lower.ends_with(" a")
            || lower.ends_with(" to")
            || lower.ends_with(":")
    };

    open_code_fence || (no_terminal_punct && mid_clause_end)
}

/// Signals that the response represents a complete, finished answer.
fn looks_complete(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();

    // Explicit completion phrases.
    let completion_phrases = [
        "let me know if",
        "feel free to ask",
        "hope this helps",
        "hope that helps",
        "if you have any questions",
        "is there anything else",
        "does this help",
        "please let me know",
        "happy to help",
    ];
    if completion_phrases.iter().any(|p| lower.contains(p)) {
        return true;
    }

    // Ends with typical sentence-ending punctuation.
    let last_char = text.chars().last().unwrap_or(' ');
    matches!(last_char, '.' | '!' | '?')
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_complete() {
        let r = detect_stop_reason("The function is now fixed. Let me know if you need anything else.", None);
        assert_eq!(r, StopReason::Complete);
    }

    #[test]
    fn test_detect_self_interrupted() {
        let r = detect_stop_reason("Here's part one of the refactor. I'll continue with the tests next.", None);
        assert_eq!(r, StopReason::SelfInterrupted);
    }

    #[test]
    fn test_detect_truncated_open_fence() {
        let r = detect_stop_reason("Here is the code:\n```rust\nfn main() {", None);
        assert_eq!(r, StopReason::Truncated);
    }

    #[test]
    fn test_detect_rate_limited_via_event() {
        let r = detect_stop_reason("", Some("rate_limit_error"));
        assert_eq!(r, StopReason::RateLimited);
    }

    #[test]
    fn test_should_auto_continue_truncated() {
        assert!(should_auto_continue(StopReason::Truncated));
    }

    #[test]
    fn test_should_not_auto_continue_complete() {
        assert!(!should_auto_continue(StopReason::Complete));
    }

    #[test]
    fn test_should_not_auto_continue_context_full() {
        assert!(!should_auto_continue(StopReason::ContextFull));
    }

    #[test]
    fn test_detect_empty_unknown() {
        let r = detect_stop_reason("", None);
        assert_eq!(r, StopReason::Unknown);
    }
}
