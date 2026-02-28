// SPDX-License-Identifier: MIT
// Session search unit tests (Sprint GG, SS.7).

use clawd::ipc::handlers::search::sanitize_fts_query_pub;

// Note: FTS5 SQL tests require a live database and are integration tests.
// These unit tests focus on the query sanitizer and response shape logic.

// ─── FTS query sanitizer ──────────────────────────────────────────────────────

#[test]
fn sanitize_strips_parentheses() {
    let q = sanitize_fts_query_pub("hello (world)");
    assert!(!q.contains('('));
    assert!(!q.contains(')'));
}

#[test]
fn sanitize_strips_caret() {
    let q = sanitize_fts_query_pub("test^2");
    assert!(!q.contains('^'));
}

#[test]
fn sanitize_wraps_in_quotes() {
    let q = sanitize_fts_query_pub("code completion");
    assert!(q.starts_with('"'), "must start with double-quote");
    assert!(q.ends_with('"'), "must end with double-quote");
}

#[test]
fn sanitize_preserves_plain_text() {
    let q = sanitize_fts_query_pub("code completion engine");
    assert!(q.contains("code completion engine"));
}

#[test]
fn sanitize_empty_query() {
    let q = sanitize_fts_query_pub("");
    // Should produce a valid (if empty) quoted string.
    assert_eq!(q, "\"\"");
}

#[test]
fn sanitize_unicode_preserved() {
    let q = sanitize_fts_query_pub("über résumé");
    assert!(q.contains("über"));
    assert!(q.contains("résumé"));
}
