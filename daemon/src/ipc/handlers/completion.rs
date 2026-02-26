// SPDX-License-Identifier: MIT
// completion.complete RPC handler (Sprint GG, CC.2).
//
// Wraps the completion engine: extracts repo context, builds the FIM prompt,
// checks the LRU cache, and returns a CompletionResponse.

use crate::completion::cache::{CacheEntry, CompletionCache};
use crate::completion::context::extract_context;
use crate::completion::engine::{
    build_fim_prompt, extract_completion_text, truncate_prefix, truncate_suffix, CompletionRequest,
    CompletionResponse, Insertion,
};
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Mutex;
use tracing::debug;

/// Shared completion cache.  Wrapped in a Mutex for interior mutability.
static CACHE: std::sync::OnceLock<Mutex<CompletionCache>> = std::sync::OnceLock::new();

fn cache() -> &'static Mutex<CompletionCache> {
    CACHE.get_or_init(|| Mutex::new(CompletionCache::new(256)))
}

/// `completion.complete` — fill-in-middle code completion.
///
/// Parameters (JSON):
/// ```json
/// {
///   "filePath": "path/to/file.rs",
///   "prefix": "text before cursor",
///   "suffix": "text after cursor",
///   "cursorLine": 10,
///   "cursorCol": 4,
///   "fileContent": "full file content"
/// }
/// ```
///
/// Returns:
/// ```json
/// {
///   "insertions": [{ "text": "...", "startLine": 10, "endLine": 10, "confidence": 0.9 }],
///   "source": "cache" | "provider"
/// }
/// ```
pub async fn complete(params: Value, ctx: &AppContext) -> Result<Value> {
    #[derive(serde::Deserialize)]
    struct Params {
        #[serde(rename = "filePath")]
        file_path: String,
        prefix: String,
        suffix: String,
        #[serde(rename = "cursorLine", default)]
        cursor_line: usize,
        #[serde(rename = "cursorCol", default)]
        cursor_col: usize,
        #[serde(rename = "fileContent", default)]
        file_content: String,
        #[serde(rename = "sessionId", default)]
        session_id: String,
    }

    let p: Params = serde_json::from_value(params)?;

    // ── 1. Cache lookup ──────────────────────────────────────────────────────
    let cache_key = CompletionCache::cache_key(&p.prefix, &p.suffix);
    if let Ok(mut c) = cache().lock() {
        if let Some(entry) = c.get(&cache_key) {
            debug!(file = %p.file_path, "completion cache hit");
            let insertions = entry.insertions.clone();
            let resp = CompletionResponse {
                insertions,
                source: "cache".to_string(),
            };
            return Ok(json!(resp));
        }
    }

    // ── 2. Build FIM prompt with optional repo context ───────────────────────
    const MAX_PREFIX: usize = 4096;
    const MAX_SUFFIX: usize = 2048;

    let prefix = truncate_prefix(&p.prefix, MAX_PREFIX);
    let suffix = truncate_suffix(&p.suffix, MAX_SUFFIX);

    let context_block = if !p.file_content.is_empty() {
        extract_context(&p.file_content, p.cursor_line, &p.file_path)
    } else {
        String::new()
    };

    let fim_prompt = if context_block.is_empty() {
        build_fim_prompt(prefix, suffix, &p.file_path)
    } else {
        format!("{context_block}\n{}", build_fim_prompt(prefix, suffix, &p.file_path))
    };

    debug!(
        file = %p.file_path,
        cursor = ?(p.cursor_line, p.cursor_col),
        prompt_len = fim_prompt.len(),
        "completion.complete → provider"
    );

    // ── 3. Call the provider via an existing session (or ephemeral) ──────────
    // If no sessionId provided, create a one-shot ephemeral session with
    // the first available provider.  For now we use the session directly.
    let response_text = if !p.session_id.is_empty() {
        match ctx.session_manager.send_message(&p.session_id, &fim_prompt, ctx).await {
            Ok(msg) => msg.content.clone(),
            Err(e) => {
                return Err(anyhow::anyhow!("provider error: {e}"));
            }
        }
    } else {
        // No session — return empty response rather than creating a session.
        // The client should pass a sessionId for actual completions.
        return Ok(json!({ "insertions": [], "source": "no_session" }));
    };

    let completion_text = extract_completion_text(&response_text);
    let line_count = completion_text.lines().count().max(1);

    let insertion = Insertion {
        text: completion_text,
        start_line: p.cursor_line,
        end_line: p.cursor_line + line_count - 1,
        confidence: 0.9,
    };

    // ── 4. Store in cache ────────────────────────────────────────────────────
    if let Ok(mut c) = cache().lock() {
        c.insert(
            cache_key,
            CacheEntry {
                insertions: vec![insertion.clone()],
                created_at: std::time::Instant::now(),
            },
        );
    }

    let resp = CompletionResponse {
        insertions: vec![insertion],
        source: "provider".to_string(),
    };
    Ok(json!(resp))
}
