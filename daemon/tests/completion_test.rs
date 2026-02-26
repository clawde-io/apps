// SPDX-License-Identifier: MIT
// Completion engine unit tests (Sprint GG, CC.9).

use clawd::completion::cache::CompletionCache;
use clawd::completion::engine::{
    build_fim_prompt, detect_language, extract_completion_text, truncate_prefix, truncate_suffix,
    FIM_MIDDLE_TOKEN, FIM_PREFIX_TOKEN, FIM_SUFFIX_TOKEN,
};
use clawd::completion::context::extract_context;

// ─── FIM format ───────────────────────────────────────────────────────────────

#[test]
fn fim_format_correct() {
    let prompt = build_fim_prompt("let x = ", ";", "main.rs");
    // Must contain all three FIM tokens in the right order.
    let pi = prompt.find(FIM_PREFIX_TOKEN).unwrap();
    let si = prompt.find(FIM_SUFFIX_TOKEN).unwrap();
    let mi = prompt.find(FIM_MIDDLE_TOKEN).unwrap();
    assert!(pi < si, "prefix token must come before suffix token");
    assert!(si < mi, "suffix token must come before middle token");
}

#[test]
fn fim_contains_prefix_and_suffix_text() {
    let prompt = build_fim_prompt("let x = ", ";", "main.rs");
    assert!(prompt.contains("let x = "));
    assert!(prompt.contains(";"));
}

#[test]
fn fim_language_hint_in_prompt() {
    let prompt = build_fim_prompt("fn main() {", "}", "lib.rs");
    assert!(prompt.contains("Rust"), "language hint must appear in FIM prompt");
}

// ─── Cache ────────────────────────────────────────────────────────────────────

#[test]
fn cache_miss_then_hit() {
    let mut cache = CompletionCache::new(16);
    let key = CompletionCache::cache_key("prefix_text", "suffix_text");

    // First access is a miss.
    assert!(cache.get(&key).is_none());
    assert_eq!(cache.misses, 1);
    assert_eq!(cache.hits, 0);

    // Insert then access is a hit.
    cache.insert(
        key.clone(),
        clawd::completion::cache::CacheEntry {
            insertions: vec![],
            created_at: std::time::Instant::now(),
        },
    );
    assert!(cache.get(&key).is_some());
    assert_eq!(cache.hits, 1);
}

#[test]
fn cache_key_same_for_identical_input() {
    let k1 = CompletionCache::cache_key("hello world", "end");
    let k2 = CompletionCache::cache_key("hello world", "end");
    assert_eq!(k1, k2, "cache key must be deterministic");
}

#[test]
fn cache_key_differs_for_different_suffix() {
    let k1 = CompletionCache::cache_key("same_prefix", "suffix_a");
    let k2 = CompletionCache::cache_key("same_prefix", "suffix_b");
    assert_ne!(k1, k2);
}

// ─── Context extraction ───────────────────────────────────────────────────────

#[test]
fn context_extracts_rust_imports() {
    let src = "use std::io;\nuse anyhow::Result;\n\nfn run() -> Result<()> {\n    Ok(())\n}";
    let ctx = extract_context(src, 4, "lib.rs");
    assert!(ctx.contains("use std::io;"));
    assert!(ctx.contains("use anyhow::Result;"));
}

#[test]
fn context_empty_for_empty_file() {
    let ctx = extract_context("", 0, "file.rs");
    assert!(ctx.is_empty());
}

// ─── Truncation ───────────────────────────────────────────────────────────────

#[test]
fn prefix_truncated_from_right() {
    let s = "abcdefgh";
    assert_eq!(truncate_prefix(s, 4), "efgh");
    assert_eq!(truncate_prefix(s, 100), "abcdefgh");
}

#[test]
fn suffix_truncated_from_left() {
    let s = "abcdefgh";
    assert_eq!(truncate_suffix(s, 4), "abcd");
    assert_eq!(truncate_suffix(s, 100), "abcdefgh");
}

// ─── Text extraction ──────────────────────────────────────────────────────────

#[test]
fn extract_strips_markdown_fence() {
    let raw = "```rust\nfn foo() {}\n```";
    assert_eq!(extract_completion_text(raw), "fn foo() {}");
}

#[test]
fn extract_no_fence_passthrough() {
    let raw = "fn foo() {}";
    assert_eq!(extract_completion_text(raw), "fn foo() {}");
}

#[test]
fn extract_trims_whitespace() {
    let raw = "  \n  let x = 1;\n  ";
    assert_eq!(extract_completion_text(raw), "let x = 1;");
}

// ─── Language detection ───────────────────────────────────────────────────────

#[test]
fn language_detection_known_extensions() {
    assert_eq!(detect_language("main.rs"), "Rust");
    assert_eq!(detect_language("app.dart"), "Dart");
    assert_eq!(detect_language("index.ts"), "TypeScript");
    assert_eq!(detect_language("script.py"), "Python");
    assert_eq!(detect_language("main.go"), "Go");
    assert_eq!(detect_language("Main.java"), "Java");
}

#[test]
fn language_detection_unknown() {
    assert_eq!(detect_language("file.xyz"), "plaintext");
    assert_eq!(detect_language("noextension"), "plaintext");
}
