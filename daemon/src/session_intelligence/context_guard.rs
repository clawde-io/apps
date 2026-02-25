// SPDX-License-Identifier: MIT
//! Context window guard + compression (SI.T02–T03).
//!
//! Monitors the total estimated token count of a session's messages against the
//! active model's context limit.  Fires events at two thresholds:
//!
//! * **Warning** (≥ 90 %) — `warning.contextNearFull` broadcast event.
//! * **Critical** (≥ 95 %) — `warning.contextFull`; caller should summarise
//!   or drop oldest non-pinned messages before the next turn.
//!
//! Compression strategy (SI.T03):  When the context is critical the caller may
//! invoke `compress_messages` which builds a trimmed message list by:
//!   1. Keeping the system prompt and all pinned messages.
//!   2. Keeping the last `keep_recent` non-pinned messages verbatim.
//!   3. Replacing older non-pinned messages with a short "[N older messages
//!      omitted]" sentinel.

use crate::intelligence::context::{estimate_tokens, ContextMessage};

// ─── Model context limits ─────────────────────────────────────────────────────

/// Well-known model context window sizes (input tokens).
///
/// These are conservative lower-bounds — we want to stay *within* the limit,
/// not verify whether the model actually supports the advertised context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelLimit {
    /// Claude Opus / Sonnet / Haiku — 200 000 token window.
    Claude200k,
    /// GPT-4o / GPT-4 Turbo — 128 000 token window.
    Gpt4_128k,
    /// GPT-4 (original) — 8 000 token window.
    Gpt4_8k,
    /// Codex / GPT-3.5 — 16 000 token window.
    Codex16k,
    /// Cursor (unknown; use a safe conservative default).
    CursorDefault,
    /// Custom limit specified in tokens.
    Custom(usize),
}

impl ModelLimit {
    /// Maximum input tokens for the model.
    pub fn max_tokens(self) -> usize {
        match self {
            Self::Claude200k => 200_000,
            Self::Gpt4_128k => 128_000,
            Self::Gpt4_8k => 8_000,
            Self::Codex16k => 16_000,
            Self::CursorDefault => 64_000,
            Self::Custom(n) => n,
        }
    }

    /// Derive from a provider name string (case-insensitive).
    pub fn from_provider(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "claude" | "claude-code" | "anthropic" => Self::Claude200k,
            "gpt-4o" | "gpt4o" | "gpt-4-turbo" => Self::Gpt4_128k,
            "gpt-4" | "gpt4" => Self::Gpt4_8k,
            "codex" | "gpt-3.5" | "gpt3.5" => Self::Codex16k,
            "cursor" => Self::CursorDefault,
            _ => Self::Claude200k, // safe default
        }
    }
}

// ─── Context status ──────────────────────────────────────────────────────────

/// Result of a context health check.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextStatus {
    /// Token usage is safely within budget.
    Ok {
        used_tokens: usize,
        max_tokens: usize,
        percent: u8,
    },
    /// Usage ≥ 90 % — warn the user but do not force compression yet.
    Warning {
        used_tokens: usize,
        max_tokens: usize,
        percent: u8,
    },
    /// Usage ≥ 95 % — compression recommended before next AI turn.
    Critical {
        used_tokens: usize,
        max_tokens: usize,
        percent: u8,
    },
}

impl ContextStatus {
    /// Return the usage percentage (0–100).
    pub fn percent(&self) -> u8 {
        match self {
            Self::Ok { percent, .. }
            | Self::Warning { percent, .. }
            | Self::Critical { percent, .. } => *percent,
        }
    }

    /// `true` when the status is `Warning` or `Critical`.
    pub fn is_elevated(&self) -> bool {
        !matches!(self, Self::Ok { .. })
    }
}

// ─── Guard ───────────────────────────────────────────────────────────────────

/// Check whether the session's accumulated token count is safe.
///
/// `total_tokens` is the sum of `estimate_tokens(content)` for every message
/// stored for the session (available from `messages.token_count` column).
pub fn check_context_health(total_tokens: usize, limit: ModelLimit) -> ContextStatus {
    let max = limit.max_tokens();
    // Use ceiling division to avoid false-safe results on large values.
    let percent = ((total_tokens as u64 * 100).div_ceil(max as u64)).min(100) as u8;

    if percent >= 95 {
        ContextStatus::Critical {
            used_tokens: total_tokens,
            max_tokens: max,
            percent,
        }
    } else if percent >= 90 {
        ContextStatus::Warning {
            used_tokens: total_tokens,
            max_tokens: max,
            percent,
        }
    } else {
        ContextStatus::Ok {
            used_tokens: total_tokens,
            max_tokens: max,
            percent,
        }
    }
}

// ─── Compression ─────────────────────────────────────────────────────────────

/// Compress a message list by retaining only the system prompt, pinned
/// messages, and the most recent `keep_recent` non-pinned messages.
///
/// Dropped messages are replaced with a single sentinel entry so the model
/// knows history was elided.
pub fn compress_messages(messages: &[ContextMessage], keep_recent: usize) -> Vec<ContextMessage> {
    // Partition: pinned/system vs regular.
    let mut system_and_pinned: Vec<ContextMessage> = Vec::new();
    let mut regular: Vec<ContextMessage> = Vec::new();

    for msg in messages {
        if msg.pinned || msg.role == "system" {
            system_and_pinned.push(msg.clone());
        } else {
            regular.push(msg.clone());
        }
    }

    let total_regular = regular.len();
    let drop_count = total_regular.saturating_sub(keep_recent);

    if drop_count == 0 {
        // Nothing to compress.
        return messages.to_vec();
    }

    // Build result: system/pinned first, then sentinel, then kept messages.
    let mut result = system_and_pinned;

    let sentinel = ContextMessage {
        role: "system".to_owned(),
        content: format!(
            "[{drop_count} earlier message{} omitted to stay within context window]",
            if drop_count == 1 { "" } else { "s" }
        ),
        pinned: false,
    };
    result.push(sentinel);

    // Append the most-recent `keep_recent` regular messages in original order.
    let start = total_regular - keep_recent.min(total_regular);
    result.extend_from_slice(&regular[start..]);

    result
}

/// Estimate total tokens across a slice of messages.
pub fn total_message_tokens(messages: &[ContextMessage]) -> usize {
    messages
        .iter()
        .map(|m| estimate_tokens(&m.role) + estimate_tokens(&m.content) + 4)
        .sum()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str, pinned: bool) -> ContextMessage {
        ContextMessage {
            role: role.to_owned(),
            content: content.to_owned(),
            pinned,
        }
    }

    #[test]
    fn test_check_ok() {
        let status = check_context_health(10_000, ModelLimit::Claude200k);
        assert!(matches!(status, ContextStatus::Ok { percent, .. } if percent < 90));
    }

    #[test]
    fn test_check_warning() {
        // 180k tokens out of 200k = 90%
        let status = check_context_health(180_000, ModelLimit::Claude200k);
        assert!(matches!(status, ContextStatus::Warning { .. }));
    }

    #[test]
    fn test_check_critical() {
        // 191k out of 200k > 95%
        let status = check_context_health(191_000, ModelLimit::Claude200k);
        assert!(matches!(status, ContextStatus::Critical { .. }));
    }

    #[test]
    fn test_compress_drops_oldest() {
        let messages = vec![
            msg("system", "You are helpful.", false),
            msg("user", "old message 1", false),
            msg("user", "old message 2", false),
            msg("user", "recent message", false),
        ];
        let compressed = compress_messages(&messages, 1);
        // system + sentinel + recent
        assert_eq!(compressed.len(), 3);
        assert!(compressed.iter().any(|m| m.content.contains("omitted")));
        assert!(compressed.iter().any(|m| m.content == "recent message"));
    }

    #[test]
    fn test_compress_keeps_pinned() {
        let messages = vec![
            msg("system", "sys", false),
            msg("user", "pinned context", true),
            msg("user", "old", false),
            msg("user", "recent", false),
        ];
        let compressed = compress_messages(&messages, 1);
        // pinned must survive
        assert!(compressed.iter().any(|m| m.content == "pinned context"));
    }

    #[test]
    fn test_compress_noop_when_few_messages() {
        let messages = vec![msg("system", "sys", false), msg("user", "hello", false)];
        let compressed = compress_messages(&messages, 10);
        assert_eq!(compressed.len(), messages.len());
    }

    #[test]
    fn test_model_limit_from_provider() {
        assert_eq!(ModelLimit::from_provider("claude").max_tokens(), 200_000);
        assert_eq!(ModelLimit::from_provider("cursor").max_tokens(), 64_000);
    }
}
