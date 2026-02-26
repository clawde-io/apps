// memory/injector.rs — Build memory prefix for AI context injection.
//
// Sprint OO ME.2: `build_memory_prefix()` — token budget, weight sort, XML wrap.
//
// Memory entries are injected into the AI system prompt as:
//   <clawd_memory>
//   [scope: global]
//   preferences.language = "Rust, TypeScript"
//   project.stack = "Next.js + Postgres"
//   ...
//   </clawd_memory>
//
// Token budget is approximated at ~4 chars per token.

use crate::memory::store::MemoryEntry;

const CHARS_PER_TOKEN: usize = 4;
const OVERHEAD_CHARS: usize = 80; // XML tags + scope headers

/// Build the memory context prefix to inject into the AI system prompt.
///
/// - `entries`: All memory entries (global + project scope), pre-sorted by weight DESC.
/// - `token_budget`: Max tokens to use for memory. Default: 500.
///
/// Returns an XML-wrapped string ready to prepend to the system prompt.
pub fn build_memory_prefix(entries: &[MemoryEntry], token_budget: usize) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let char_budget = (token_budget * CHARS_PER_TOKEN).saturating_sub(OVERHEAD_CHARS);
    let mut lines: Vec<String> = Vec::new();
    let mut chars_used = 0;

    for entry in entries {
        let line = format!("{} = {:?}", entry.key, entry.value);
        if chars_used + line.len() > char_budget {
            // Budget exceeded — stop adding entries
            break;
        }
        lines.push(line.clone());
        chars_used += line.len() + 1; // +1 for newline
    }

    if lines.is_empty() {
        return String::new();
    }

    format!("<clawd_memory>\n{}\n</clawd_memory>\n", lines.join("\n"))
}

/// Estimated token count for a memory prefix string.
pub fn estimate_tokens(prefix: &str) -> usize {
    prefix.len().div_ceil(CHARS_PER_TOKEN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::MemoryEntry;

    fn make_entry(key: &str, value: &str, weight: i64) -> MemoryEntry {
        MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            scope: "global".to_string(),
            key: key.to_string(),
            value: value.to_string(),
            weight,
            source: "user".to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn test_empty_entries_returns_empty_string() {
        let prefix = build_memory_prefix(&[], 500);
        assert!(prefix.is_empty());
    }

    #[test]
    fn test_prefix_contains_xml_tags() {
        let entries = vec![make_entry("lang", "Rust", 8)];
        let prefix = build_memory_prefix(&entries, 500);
        assert!(prefix.starts_with("<clawd_memory>"));
        assert!(prefix.ends_with("</clawd_memory>\n"));
    }

    #[test]
    fn test_budget_limits_entries() {
        // Budget of 5 tokens = ~20 chars — should cut off most entries
        let entries = vec![
            make_entry("preferences.language", "Rust, TypeScript, Python", 10),
            make_entry("preferences.style", "terse, direct", 9),
            make_entry("project.stack", "Next.js + Postgres + Hasura", 8),
        ];
        let prefix = build_memory_prefix(&entries, 5);
        // With a tiny budget, should have fewer entries than total
        let line_count = prefix.lines().count();
        assert!(line_count < entries.len() + 2); // +2 for opening/closing tags
    }

    #[test]
    fn test_token_estimate() {
        let text = "hello world"; // 11 chars / 4 ≈ 3 tokens
        let tokens = estimate_tokens(text);
        assert!((2..=4).contains(&tokens));
    }
}
