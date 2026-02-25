// SPDX-License-Identifier: MIT
//! Context window optimizer — sliding window with pinned messages.
//!
//! Builds the message list that is sent to each AI call, respecting a token
//! budget.  The algorithm:
//!
//! 1. Always include the system prompt (first message, role = "system").
//! 2. Always include any pinned messages (role = "system" or explicitly marked).
//! 3. Fill remaining budget with the most recent non-pinned messages,
//!    newest first.
//! 4. If the most recent message alone exceeds the budget, truncate its
//!    content to fit.
//!
//! Token counting uses a fast heuristic: `ceil(chars / 4)`.  This is
//! intentionally approximate — the goal is to stay well within the model's
//! context window, not to count exactly.

/// A single message as the context optimizer sees it.
#[derive(Debug, Clone)]
pub struct ContextMessage {
    /// "system", "user", or "assistant".
    pub role: String,
    /// Raw message content (may be truncated by the optimizer).
    pub content: String,
    /// If true, this message is always included regardless of budget.
    pub pinned: bool,
}

/// Parameters for context optimization.
pub struct ContextConfig {
    /// Maximum tokens allowed for the full messages array.
    pub max_tokens: usize,
    /// Reserve this many tokens for the model's response.  Reduces the
    /// effective budget for the input messages.
    pub response_reserve_tokens: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 100_000,
            response_reserve_tokens: 4_096,
        }
    }
}

/// Estimate token count from a string using the 4-chars-per-token heuristic.
///
/// Errs on the side of over-counting (ceiling division) to avoid going over
/// budget.
#[inline]
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Trim `text` so that its estimated token count does not exceed `max_tokens`.
///
/// Truncation appends `…` to signal that the content was cut.  If `max_tokens`
/// is 0 the function returns an empty string.
pub fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    if max_tokens == 0 {
        return String::new();
    }
    let char_limit = max_tokens * 4;
    if text.len() <= char_limit {
        return text.to_owned();
    }
    // Find a clean UTF-8 boundary near the limit.
    let boundary = text
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i < char_limit.saturating_sub(3))
        .last()
        .unwrap_or(0);
    format!("{}…", &text[..boundary])
}

/// Build an optimized message list that fits within `config.max_tokens`.
///
/// The returned list preserves message ordering (system first, then
/// chronological).  If messages must be dropped, the oldest non-pinned,
/// non-system messages are dropped first.
///
/// # Arguments
///
/// * `messages` — Full ordered list, oldest first.
/// * `config` — Token budget and reserve settings.
pub fn optimize_context(
    messages: &[ContextMessage],
    config: &ContextConfig,
) -> Vec<ContextMessage> {
    let budget = config
        .max_tokens
        .saturating_sub(config.response_reserve_tokens);

    // Separate pinned/system messages from regular ones.
    let mut pinned: Vec<&ContextMessage> = Vec::new();
    let mut regular: Vec<&ContextMessage> = Vec::new();

    for msg in messages {
        if msg.pinned || msg.role == "system" {
            pinned.push(msg);
        } else {
            regular.push(msg);
        }
    }

    // Calculate tokens consumed by pinned messages.
    let pinned_tokens: usize = pinned
        .iter()
        .map(|m| estimate_tokens(&m.role) + estimate_tokens(&m.content) + 4)
        .sum();

    let mut remaining = budget.saturating_sub(pinned_tokens);

    // Walk regular messages from newest to oldest, collecting as many as fit.
    let mut selected: Vec<&ContextMessage> = Vec::new();
    for msg in regular.iter().rev() {
        let cost = estimate_tokens(&msg.role) + estimate_tokens(&msg.content) + 4;
        if cost <= remaining {
            selected.push(msg);
            remaining -= cost;
        }
        // Stop once the remaining budget is negligible.
        if remaining < 16 {
            break;
        }
    }

    // Reverse so chronological order is restored.
    selected.reverse();

    // Merge pinned + selected, preserving original order.
    let selected_ptrs: std::collections::HashSet<*const ContextMessage> = selected
        .iter()
        .map(|m| *m as *const ContextMessage)
        .collect();

    let mut result: Vec<ContextMessage> = messages
        .iter()
        .filter(|m| {
            m.pinned || m.role == "system" || selected_ptrs.contains(&(*m as *const ContextMessage))
        })
        .cloned()
        .collect();

    // If the most recent assistant/user message alone exceeds what's left,
    // truncate its content.
    if let Some(last) = result.last_mut() {
        if last.role != "system" && !last.pinned {
            let full_cost = estimate_tokens(&last.content);
            if full_cost > budget {
                last.content = truncate_to_tokens(&last.content, budget);
            }
        }
    }

    result
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(role: &str, content: &str, pinned: bool) -> ContextMessage {
        ContextMessage {
            role: role.to_owned(),
            content: content.to_owned(),
            pinned,
        }
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_four_chars() {
        // 4 chars = 1 token
        assert_eq!(estimate_tokens("abcd"), 1);
    }

    #[test]
    fn test_estimate_tokens_five_chars() {
        // 5 chars → ceil(5/4) = 2
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn test_truncate_exact_fit() {
        let s = "abcd"; // 1 token
        assert_eq!(truncate_to_tokens(s, 1), s);
    }

    #[test]
    fn test_truncate_over_limit() {
        let s = "a".repeat(100);
        let result = truncate_to_tokens(&s, 5); // 5 tokens = 20 chars
        assert!(result.len() < s.len(), "should be shorter");
        assert!(result.ends_with('…'), "should end with ellipsis");
    }

    #[test]
    fn test_truncate_zero_limit() {
        let result = truncate_to_tokens("hello", 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_optimize_keeps_all_within_budget() {
        let messages = vec![
            make_msg("system", "You are a helpful assistant.", false),
            make_msg("user", "Hello!", false),
            make_msg("assistant", "Hi there!", false),
        ];
        let config = ContextConfig {
            max_tokens: 10_000,
            response_reserve_tokens: 500,
        };
        let result = optimize_context(&messages, &config);
        assert_eq!(result.len(), 3, "all 3 messages should fit");
    }

    #[test]
    fn test_optimize_drops_old_messages_first() {
        // Create a tight budget so only system + last user message fit.
        let system_content = "sys";
        let old_user = "a".repeat(1000);
        let new_user = "new question";

        let messages = vec![
            make_msg("system", system_content, false),
            make_msg("user", &old_user, false),
            make_msg("user", new_user, false),
        ];

        // Budget: system (~1 tok) + new_user (~3 tok) + overhead = ~20.
        // old_user (250 tok) should be dropped.
        let config = ContextConfig {
            max_tokens: 30,
            response_reserve_tokens: 4,
        };
        let result = optimize_context(&messages, &config);

        // System must be there.
        assert!(result.iter().any(|m| m.role == "system"));
        // New user message must be there.
        assert!(result.iter().any(|m| m.content == new_user));
        // Old (1000-char) message must be dropped.
        assert!(!result.iter().any(|m| m.content == old_user));
    }

    #[test]
    fn test_optimize_pinned_always_included() {
        let pinned_msg = make_msg("user", "important pinned message", true);
        let other = make_msg("user", "regular message", false);

        let messages = vec![pinned_msg, other];
        // Very tight budget — only the pinned message can fit.
        let config = ContextConfig {
            max_tokens: 10,
            response_reserve_tokens: 0,
        };
        let result = optimize_context(&messages, &config);
        assert!(
            result.iter().any(|m| m.pinned),
            "pinned message must survive budget cuts"
        );
    }

    #[test]
    fn test_optimize_empty_input() {
        let result = optimize_context(&[], &ContextConfig::default());
        assert!(result.is_empty());
    }
}
