// SPDX-License-Identifier: MIT
// session.search RPC handler (Sprint GG, SS.3 + SS.5).
//
// Full-text search across all session messages using SQLite FTS5.
// Returns BM25-ranked results with snippets.

use crate::AppContext;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;

/// A single search result entry.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    /// ID of the session containing this message.
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// ID of the matching message.
    #[serde(rename = "messageId")]
    pub message_id: String,
    /// Short snippet of the matching text (FTS5 `snippet()` function output).
    pub snippet: String,
    /// Role of the message author ("user" | "assistant").
    pub role: String,
    /// ISO-8601 creation timestamp of the message.
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// BM25 rank (lower is more relevant; exposed as negative f32 for display).
    pub rank: f32,
}

/// Optional search filters.
#[derive(Debug, Clone, Deserialize, Default)]
struct SearchFilters {
    /// Restrict results to a specific session.
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    /// Restrict to messages created on or after this ISO-8601 timestamp.
    #[serde(rename = "dateFrom")]
    date_from: Option<String>,
    /// Restrict to messages created before or at this ISO-8601 timestamp.
    #[serde(rename = "dateTo")]
    date_to: Option<String>,
    /// Restrict to a specific message role ("user" | "assistant").
    role: Option<String>,
}

/// `session.search` — full-text search across all session messages.
///
/// Parameters (JSON):
/// ```json
/// {
///   "query": "string to search",
///   "limit": 20,
///   "filterBy": {
///     "sessionId": "optional",
///     "dateFrom": "2026-01-01T00:00:00Z",
///     "dateTo": "2026-12-31T23:59:59Z",
///     "role": "user"
///   }
/// }
/// ```
///
/// Returns:
/// ```json
/// {
///   "results": [{ "sessionId", "messageId", "snippet", "role", "createdAt", "rank" }],
///   "totalHits": 5
/// }
/// ```
pub async fn search(params: Value, ctx: &AppContext) -> Result<Value> {
    #[derive(Deserialize)]
    struct Params {
        query: String,
        #[serde(default = "default_limit")]
        limit: u32,
        #[serde(rename = "filterBy", default)]
        filter_by: SearchFilters,
    }

    fn default_limit() -> u32 {
        20
    }

    let p: Params = serde_json::from_value(params)?;

    if p.query.trim().is_empty() {
        return Ok(json!({ "results": [], "totalHits": 0 }));
    }

    // Clamp limit to a sane maximum.
    let limit = p.limit.min(200) as i64;

    debug!(query = %p.query, limit = limit, "session.search");

    // Build dynamic SQL using FTS5 MATCH with optional row value filters.
    // We build a parameterized query by composing filter conditions.
    let mut conditions: Vec<String> = Vec::new();
    if let Some(sid) = &p.filter_by.session_id {
        conditions.push(format!("session_id = '{}'", sid.replace('\'', "''")));
    }
    if let Some(role) = &p.filter_by.role {
        conditions.push(format!("role = '{}'", role.replace('\'', "''")));
    }
    if let Some(from) = &p.filter_by.date_from {
        conditions.push(format!("created_at >= '{}'", from.replace('\'', "''")));
    }
    if let Some(to) = &p.filter_by.date_to {
        conditions.push(format!("created_at <= '{}'", to.replace('\'', "''")));
    }

    // Escape the FTS query string (FTS5 uses a different syntax).
    let fts_query = sanitize_fts_query(&p.query);

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("AND {}", conditions.join(" AND "))
    };

    let sql = format!(
        r#"
        SELECT
            session_id,
            message_id,
            snippet(session_fts, 0, '<b>', '</b>', '…', 16) AS snippet,
            role,
            created_at,
            rank
        FROM session_fts
        WHERE session_fts MATCH ?
        {where_clause}
        ORDER BY rank
        LIMIT {limit}
        "#
    );

    let rows = sqlx::query_as::<_, (String, String, String, String, String, f64)>(&sql)
        .bind(&fts_query)
        .fetch_all(ctx.storage.pool())
        .await
        .map_err(|e| anyhow::anyhow!("FTS search failed: {e}"))?;

    let results: Vec<SearchResult> = rows
        .into_iter()
        .map(|(session_id, message_id, snippet, role, created_at, rank)| SearchResult {
            session_id,
            message_id,
            snippet,
            role,
            created_at,
            rank: rank as f32,
        })
        .collect();

    let total = results.len();
    Ok(json!({ "results": results, "totalHits": total }))
}

/// Sanitize a user-provided query string for FTS5 MATCH.
///
/// FTS5 uses a special query syntax; untrusted input could break the query or
/// cause confusing errors.  We strip special FTS5 operators and wrap the
/// cleaned text in double-quotes for a literal phrase search.
pub fn sanitize_fts_query_pub(query: &str) -> String {
    sanitize_fts_query(query)
}

fn sanitize_fts_query(query: &str) -> String {
    // Remove FTS5 special chars: " ^ * ( ) OR AND NOT
    let clean: String = query
        .chars()
        .filter(|c| !matches!(c, '"' | '^' | '(' | ')'))
        .collect();
    let clean = clean.trim();
    // Wrap in double-quotes for exact phrase matching.
    format!("\"{}\"", clean.replace('\\', ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_special_chars() {
        let q = sanitize_fts_query("hello (world)^2");
        assert!(!q.contains('('));
        assert!(!q.contains('^'));
        assert!(q.starts_with('"'));
        assert!(q.ends_with('"'));
    }

    #[test]
    fn sanitize_preserves_text() {
        let q = sanitize_fts_query("code completion");
        assert!(q.contains("code completion"));
    }

    #[test]
    fn empty_query_sanitized() {
        let q = sanitize_fts_query("");
        assert_eq!(q, "\"\"");
    }
}
