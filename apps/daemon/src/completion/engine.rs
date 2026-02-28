// SPDX-License-Identifier: MIT
// Code Completion Engine — core FIM engine (Sprint GG, CC.1).

use serde::{Deserialize, Serialize};

// ─── Request / Response types ─────────────────────────────────────────────────

/// Input parameters for a fill-in-middle completion request.
#[derive(Debug, Clone, Deserialize)]
pub struct CompletionRequest {
    /// Absolute path of the file being edited (language detection).
    #[serde(rename = "filePath")]
    pub file_path: String,
    /// Text immediately before the cursor.
    pub prefix: String,
    /// Text immediately after the cursor.
    pub suffix: String,
    /// 0-based line number of the cursor.
    #[serde(rename = "cursorLine", default)]
    pub cursor_line: usize,
    /// 0-based column of the cursor.
    #[serde(rename = "cursorCol", default)]
    pub cursor_col: usize,
}

/// A single inline code-completion suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insertion {
    /// The text to insert at the cursor position.
    pub text: String,
    /// 0-based start line of the suggested range (inclusive).
    #[serde(rename = "startLine")]
    pub start_line: usize,
    /// 0-based end line of the suggested range (inclusive).
    #[serde(rename = "endLine")]
    pub end_line: usize,
    /// Confidence 0.0–1.0 (provider-reported or heuristic).
    pub confidence: f32,
}

/// Response returned by the completion engine.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionResponse {
    /// Ordered list of suggestions (best first).
    pub insertions: Vec<Insertion>,
    /// Source of the completion ("cache" | "provider").
    pub source: String,
}

// ─── FIM prompt builder ───────────────────────────────────────────────────────

/// The standard fill-in-middle token sequence used by most providers.
pub const FIM_PREFIX_TOKEN: &str = "<|fim_prefix|>";
pub const FIM_SUFFIX_TOKEN: &str = "<|fim_suffix|>";
pub const FIM_MIDDLE_TOKEN: &str = "<|fim_middle|>";

/// Build a FIM prompt from the given prefix, suffix, and language hint.
///
/// Format:
/// ```text
/// <|fim_prefix|>{prefix}<|fim_suffix|>{suffix}<|fim_middle|>
/// ```
///
/// A brief instruction line is prepended so the provider returns only the
/// inserted code and no surrounding explanation.
pub fn build_fim_prompt(prefix: &str, suffix: &str, file_path: &str) -> String {
    let lang = detect_language(file_path);
    format!(
        "Complete the missing code. Language: {lang}. \
         Return ONLY the inserted text — no markdown fences, no explanation.\n\
         {FIM_PREFIX_TOKEN}{prefix}{FIM_SUFFIX_TOKEN}{suffix}{FIM_MIDDLE_TOKEN}"
    )
}

/// Truncate prefix to at most `max` characters (from the right).
pub fn truncate_prefix(prefix: &str, max: usize) -> &str {
    if prefix.len() > max {
        &prefix[prefix.len() - max..]
    } else {
        prefix
    }
}

/// Truncate suffix to at most `max` characters (from the left).
pub fn truncate_suffix(suffix: &str, max: usize) -> &str {
    if suffix.len() > max {
        &suffix[..max]
    } else {
        suffix
    }
}

/// Strip markdown code fences from a provider response, if present.
pub fn extract_completion_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(after_fence) = trimmed.strip_prefix("```") {
        let body = if let Some(nl) = after_fence.find('\n') {
            &after_fence[nl + 1..]
        } else {
            after_fence
        };
        let stripped = if let Some(end) = body.rfind("\n```") {
            &body[..end]
        } else {
            body.strip_suffix("```").unwrap_or(body)
        };
        return stripped.to_string();
    }
    trimmed.to_string()
}

/// Detect a programming language label from a file extension.
pub fn detect_language(file_path: &str) -> &'static str {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" => "Rust",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "dart" => "Dart",
        "py" | "pyw" => "Python",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "swift" => "Swift",
        "c" | "h" => "C",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "C++",
        "cs" => "C#",
        "rb" => "Ruby",
        "php" => "PHP",
        "html" | "htm" => "HTML",
        "css" | "scss" | "sass" | "less" => "CSS",
        "sql" => "SQL",
        "sh" | "bash" | "zsh" => "Shell",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "json" | "jsonc" => "JSON",
        "md" | "mdx" => "Markdown",
        "lua" => "Lua",
        "r" => "R",
        "ex" | "exs" => "Elixir",
        "hs" => "Haskell",
        _ => "plaintext",
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fim_prompt_contains_tokens() {
        let prompt = build_fim_prompt("let x = ", ";", "foo.rs");
        assert!(prompt.contains(FIM_PREFIX_TOKEN));
        assert!(prompt.contains(FIM_SUFFIX_TOKEN));
        assert!(prompt.contains(FIM_MIDDLE_TOKEN));
        assert!(prompt.contains("Rust"));
    }

    #[test]
    fn fim_token_order() {
        let prompt = build_fim_prompt("prefix", "suffix", "a.ts");
        let pi = prompt.find(FIM_PREFIX_TOKEN).unwrap();
        let si = prompt.find(FIM_SUFFIX_TOKEN).unwrap();
        let mi = prompt.find(FIM_MIDDLE_TOKEN).unwrap();
        assert!(
            pi < si && si < mi,
            "FIM tokens must be in prefix→suffix→middle order"
        );
    }

    #[test]
    fn truncate_prefix_clips_right() {
        let s = "abcdef";
        assert_eq!(truncate_prefix(s, 3), "def");
        assert_eq!(truncate_prefix(s, 100), "abcdef");
    }

    #[test]
    fn truncate_suffix_clips_left() {
        let s = "abcdef";
        assert_eq!(truncate_suffix(s, 3), "abc");
        assert_eq!(truncate_suffix(s, 100), "abcdef");
    }

    #[test]
    fn extract_strips_fences() {
        assert_eq!(
            extract_completion_text("```rust\nfn f(){}\n```"),
            "fn f(){}"
        );
        assert_eq!(extract_completion_text("fn f(){}"), "fn f(){}");
    }

    #[test]
    fn detect_language_coverage() {
        assert_eq!(detect_language("main.rs"), "Rust");
        assert_eq!(detect_language("app.dart"), "Dart");
        assert_eq!(detect_language("index.html"), "HTML");
        assert_eq!(detect_language("unknown.xyz"), "plaintext");
    }
}
