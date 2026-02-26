//! Sprint CC TC.2 — AI confidence score parsing at session complete.
//!
//! When a session turn completes, the last assistant message is scanned for
//! a confidence score in the range 0.0–1.0. The score and reasoning are
//! persisted to `agent_tasks.confidence_score` / `confidence_reasoning` for
//! the task linked to the session.

use regex::Regex;
use std::sync::OnceLock;

/// Parsed confidence result from the last AI message.
#[derive(Debug, Clone)]
pub struct ConfidenceResult {
    pub score: f64,
    pub reasoning: String,
}

/// Extract a confidence score from the last assistant message content.
///
/// Looks for patterns like:
/// - `Confidence: 0.85`
/// - `confidence score: 0.9`
/// - `0.85 — I'm fairly confident because…`
/// - `My confidence is 0.7`
///
/// Returns `None` when no confidence marker is detected.
pub fn parse_confidence(content: &str) -> Option<ConfidenceResult> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // Match "confidence[:][ ]0.N" or bare "0.N" near the word "confidence"
        Regex::new(
            r"(?i)(?:confidence[^0-9.]*|my confidence is\s*)([0-9](?:\.[0-9]+)?)"
        ).expect("confidence regex is valid")
    });

    let caps = re.captures(content)?;
    let score_str = caps.get(1)?.as_str();
    let score: f64 = score_str.parse().ok()?;

    // Clamp to [0.0, 1.0]
    let score = score.clamp(0.0, 1.0);

    // Extract the reasoning: take the sentence containing the score match.
    let reasoning = extract_reasoning_sentence(content, caps.get(0)?.start());

    Some(ConfidenceResult { score, reasoning })
}

/// Extract the sentence (or up to 200 chars) surrounding the confidence match
/// to use as the reasoning text.
fn extract_reasoning_sentence(content: &str, match_start: usize) -> String {
    // Walk back to sentence start.
    let before = &content[..match_start];
    let sentence_start = before
        .rfind(|c| c == '.' || c == '\n')
        .map(|i| i + 1)
        .unwrap_or(0);

    // Walk forward to sentence end.
    let after_match = &content[match_start..];
    let sentence_end = after_match
        .find(|c| c == '.' || c == '\n')
        .map(|i| match_start + i + 1)
        .unwrap_or(content.len());

    let sentence = &content[sentence_start..sentence_end.min(content.len())];
    let trimmed = sentence.trim().to_string();

    // Cap at 200 chars.
    if trimmed.len() > 200 {
        format!("{}…", &trimmed[..200])
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_confidence_colon_format() {
        let msg = "The refactoring is complete. Confidence: 0.85 — all tests pass.";
        let result = parse_confidence(msg).unwrap();
        assert!((result.score - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_parse_confidence_my_format() {
        let msg = "My confidence is 0.7 because there are untested edge cases.";
        let result = parse_confidence(msg).unwrap();
        assert!((result.score - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_no_confidence_marker() {
        let msg = "Done. The file has been updated successfully.";
        assert!(parse_confidence(msg).is_none());
    }

    #[test]
    fn test_score_clamped_to_one() {
        let msg = "Confidence: 1.5 — extremely confident.";
        let result = parse_confidence(msg).unwrap();
        assert!((result.score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_reasoning_extracted() {
        let msg = "All tests pass. Confidence: 0.9 — I verified every branch.";
        let result = parse_confidence(msg).unwrap();
        assert!(result.reasoning.contains("0.9"));
    }
}
