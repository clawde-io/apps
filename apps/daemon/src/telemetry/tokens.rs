//! Token counting utilities — estimates and header parsing.

use std::collections::HashMap;

// ─── Estimation ───────────────────────────────────────────────────────────────

/// Rough token estimate from raw text.
///
/// Uses the heuristic of 1 token ≈ 4 characters for English prose.
/// This is sufficient for cost estimation; exact counts come from provider
/// response headers when available (see `parse_token_headers`).
pub fn estimate_tokens(text: &str) -> u64 {
    let chars = text.len() as u64;
    chars.div_ceil(4)
}

// ─── Header parsing ───────────────────────────────────────────────────────────

/// Extract input and output token counts from AI-provider response headers.
///
/// Claude API uses:
///   `X-Request-Usage-Input-Tokens`  / `X-Request-Usage-Output-Tokens`
///
/// OpenAI / Codex uses:
///   `openai-usage-prompt-tokens` / `openai-usage-completion-tokens`
///
/// Returns `(input_tokens, output_tokens)` where either may be `None` if the
/// header is absent or cannot be parsed as an integer.
pub fn parse_token_headers(headers: &HashMap<String, String>) -> (Option<u64>, Option<u64>) {
    // Helper: try several header name variants, case-insensitive keys assumed already lowered.
    let lookup = |keys: &[&str]| -> Option<u64> {
        for key in keys {
            if let Some(v) = headers
                .get(*key)
                .or_else(|| headers.get(&key.to_lowercase()))
            {
                if let Ok(n) = v.parse::<u64>() {
                    return Some(n);
                }
            }
        }
        None
    };

    let input = lookup(&[
        "x-request-usage-input-tokens",
        "openai-usage-prompt-tokens",
        "x-input-tokens",
    ]);
    let output = lookup(&[
        "x-request-usage-output-tokens",
        "openai-usage-completion-tokens",
        "x-output-tokens",
    ]);

    (input, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn estimate_four_chars() {
        // Exactly 4 chars → 1 token.
        assert_eq!(estimate_tokens("abcd"), 1);
    }

    #[test]
    fn estimate_rounds_up() {
        // 5 chars → ceil(5/4) = 2 tokens.
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn parse_headers_claude_style() {
        let mut h = HashMap::new();
        h.insert(
            "x-request-usage-input-tokens".to_string(),
            "100".to_string(),
        );
        h.insert(
            "x-request-usage-output-tokens".to_string(),
            "200".to_string(),
        );
        let (i, o) = parse_token_headers(&h);
        assert_eq!(i, Some(100));
        assert_eq!(o, Some(200));
    }

    #[test]
    fn parse_headers_missing() {
        let h = HashMap::new();
        let (i, o) = parse_token_headers(&h);
        assert!(i.is_none());
        assert!(o.is_none());
    }
}
