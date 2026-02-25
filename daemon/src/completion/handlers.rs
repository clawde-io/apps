// SPDX-License-Identifier: MIT
// Code Completion Engine — RPC handlers (Sprint K, CC.T01–CC.T03).
//
// completion.suggest — build a fill-in-middle prompt, send it to the session's
// AI provider, and return a CompletionSuggestion.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use tracing::debug;

use super::model::{CompletionRequest, CompletionSuggestion};

/// `completion.suggest` — request a fill-in-middle code completion.
///
/// Builds a structured FIM (fill-in-middle) prompt using the prefix and suffix
/// extracted from the editor state, sends it to the active session's AI
/// provider, and returns the suggested completion text with its source range.
///
/// The prompt format follows the convention used by most providers:
///
/// ```text
/// <fim_prefix>{prefix}<fim_suffix>{suffix}<fim_middle>
/// ```
///
/// For multi-line suggestions the response may span multiple lines.  The
/// returned `start_line` / `end_line` indicate which lines would be replaced.
pub async fn suggest_completion(params: Value, ctx: &AppContext) -> Result<Value> {
    let req: CompletionRequest = serde_json::from_value(params)?;

    if req.session_id.is_empty() {
        anyhow::bail!("invalid type: sessionId must not be empty");
    }

    // Verify the session exists before building the prompt.
    let session = ctx
        .session_manager
        .get(&req.session_id)
        .await
        .map_err(|_| anyhow::anyhow!("SESSION_NOT_FOUND: session '{}' not found", req.session_id))?;

    debug!(
        session_id = %req.session_id,
        file_path = %req.file_path,
        cursor = ?( req.cursor_line, req.cursor_col),
        provider = %session.provider,
        "completion.suggest requested"
    );

    // Determine max prefix/suffix length to avoid exceeding context window.
    // These conservative limits keep the FIM payload well within any provider's window.
    const MAX_PREFIX_CHARS: usize = 4000;
    const MAX_SUFFIX_CHARS: usize = 2000;

    let prefix = if req.prefix.len() > MAX_PREFIX_CHARS {
        &req.prefix[req.prefix.len() - MAX_PREFIX_CHARS..]
    } else {
        &req.prefix
    };

    let suffix = if req.suffix.len() > MAX_SUFFIX_CHARS {
        &req.suffix[..MAX_SUFFIX_CHARS]
    } else {
        &req.suffix
    };

    // Build the fill-in-middle prompt.
    // Claude uses <fim_prefix>/<fim_suffix>/<fim_middle> XML-style tags.
    // Codex uses the same FIM format since it was popularised by Codex.
    let fim_prompt = build_fim_prompt(prefix, suffix, &req.file_path);

    // Send the FIM prompt to the session and capture the response.
    let message = ctx
        .session_manager
        .send_message(&req.session_id, &fim_prompt, ctx)
        .await?;

    // Extract the completion text from the provider response.
    let suggestion_text = extract_completion(&message.content);

    // Calculate the line range that the suggestion covers.
    let line_count = suggestion_text.lines().count().max(1);
    let suggestion = CompletionSuggestion {
        text: suggestion_text,
        start_line: req.cursor_line,
        end_line: req.cursor_line + line_count - 1,
    };

    debug!(
        session_id = %req.session_id,
        lines = line_count,
        "completion returned"
    );

    Ok(json!({
        "suggestion": suggestion,
        "lineCount": line_count,
    }))
}

/// Build a fill-in-middle prompt for the given prefix and suffix.
///
/// Includes a brief system instruction so the provider understands it should
/// only return the missing code and nothing else.
fn build_fim_prompt(prefix: &str, suffix: &str, file_path: &str) -> String {
    // Detect language hint from file extension.
    let lang = detect_language(file_path);

    format!(
        "You are a code completion engine. Complete the missing code between the prefix and suffix. \
         Return ONLY the completion text — no explanation, no markdown fences, no extra whitespace before or after.\n\
         Language: {lang}\n\n\
         <fim_prefix>{prefix}<fim_suffix>{suffix}<fim_middle>"
    )
}

/// Detect a language label from a file extension for use in the FIM prompt.
fn detect_language(file_path: &str) -> &'static str {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs"                       => "Rust",
        "ts" | "tsx"               => "TypeScript",
        "js" | "jsx" | "mjs"      => "JavaScript",
        "dart"                     => "Dart",
        "py"                       => "Python",
        "go"                       => "Go",
        "java"                     => "Java",
        "kt" | "kts"               => "Kotlin",
        "swift"                    => "Swift",
        "c" | "h"                  => "C",
        "cpp" | "cc" | "cxx" | "hpp" => "C++",
        "cs"                       => "C#",
        "rb"                       => "Ruby",
        "php"                      => "PHP",
        "html" | "htm"             => "HTML",
        "css" | "scss" | "sass"    => "CSS",
        "sql"                      => "SQL",
        "sh" | "bash"              => "Shell",
        "toml"                     => "TOML",
        "yaml" | "yml"             => "YAML",
        "json"                     => "JSON",
        "md" | "mdx"               => "Markdown",
        _                          => "plaintext",
    }
}

/// Extract the completion text from a provider response.
///
/// Providers may wrap the completion in markdown code fences or add
/// explanatory text — this function strips those wrappings when present
/// and returns the raw code.
fn extract_completion(content: &str) -> String {
    let trimmed = content.trim();

    // Strip a leading ``` fence (with optional language label) and trailing ```.
    if let Some(after_fence) = trimmed.strip_prefix("```") {
        // Skip the language label line if present.
        let body = if let Some(newline) = after_fence.find('\n') {
            &after_fence[newline + 1..]
        } else {
            after_fence
        };
        // Strip trailing ```.
        let stripped = if let Some(end) = body.rfind("\n```") {
            &body[..end]
        } else {
            body.strip_suffix("```").unwrap_or(body)
        };
        return stripped.to_string();
    }

    // No fences — return as-is.
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_rust() {
        assert_eq!(detect_language("main.rs"), "Rust");
    }

    #[test]
    fn test_detect_language_typescript() {
        assert_eq!(detect_language("app.tsx"), "TypeScript");
    }

    #[test]
    fn test_detect_language_unknown() {
        assert_eq!(detect_language("file.xyz"), "plaintext");
    }

    #[test]
    fn test_extract_completion_strips_fences() {
        let raw = "```rust\nfn foo() {}\n```";
        assert_eq!(extract_completion(raw), "fn foo() {}");
    }

    #[test]
    fn test_extract_completion_no_fences() {
        let raw = "fn foo() {}";
        assert_eq!(extract_completion(raw), "fn foo() {}");
    }

    #[test]
    fn test_build_fim_prompt_contains_tags() {
        let prompt = build_fim_prompt("let x = ", ";", "foo.rs");
        assert!(prompt.contains("<fim_prefix>let x = "));
        assert!(prompt.contains("<fim_suffix>;"));
        assert!(prompt.contains("<fim_middle>"));
        assert!(prompt.contains("Rust"));
    }
}
