// SPDX-License-Identifier: MIT
// Repo context injection for completions (Sprint GG, CC.4).
//
// Before sending a FIM prompt, prepend a compact context block containing:
//   - The file's import statements (lines starting with import/use/require/from/include)
//   - The enclosing module/class/function signature (nearest line above the cursor
//     that looks like a definition)
//
// The context block is capped at 512 tokens (approximately 2048 characters).

const MAX_CONTEXT_CHARS: usize = 2048;

/// Extract a compact context block from file content and cursor position.
///
/// Returns a string suitable for prepending to the FIM prompt, or an empty
/// string if no useful context can be extracted.
pub fn extract_context(file_content: &str, cursor_line: usize, file_path: &str) -> String {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let lines: Vec<&str> = file_content.lines().collect();
    let mut parts: Vec<String> = Vec::new();

    // ── 1. Import lines ─────────────────────────────────────────────────────
    let import_lines: Vec<&str> = lines
        .iter()
        .filter(|l| is_import_line(l, ext))
        .copied()
        .collect();
    if !import_lines.is_empty() {
        parts.push(import_lines.join("\n"));
    }

    // ── 2. Nearest enclosing definition above cursor ─────────────────────────
    let effective_cursor = cursor_line.min(lines.len().saturating_sub(1));
    if let Some(sig) = nearest_definition(&lines, effective_cursor, ext) {
        parts.push(sig);
    }

    if parts.is_empty() {
        return String::new();
    }

    let combined = parts.join("\n");
    // Cap to MAX_CONTEXT_CHARS characters.
    let truncated = if combined.len() > MAX_CONTEXT_CHARS {
        &combined[..MAX_CONTEXT_CHARS]
    } else {
        &combined
    };

    format!("// Context:\n{truncated}\n// End context\n")
}

/// Return true if the line is an import/use/require statement for the given extension.
fn is_import_line(line: &str, ext: &str) -> bool {
    let trimmed = line.trim();
    match ext {
        "rs" => trimmed.starts_with("use "),
        "ts" | "tsx" | "js" | "jsx" | "mjs" => {
            trimmed.starts_with("import ")
                || trimmed.starts_with("const ") && trimmed.contains("require(")
        }
        "dart" => trimmed.starts_with("import "),
        "py" | "pyw" => trimmed.starts_with("import ") || trimmed.starts_with("from "),
        "go" => trimmed.starts_with("import "),
        "java" | "kt" | "kts" => trimmed.starts_with("import "),
        "cs" => trimmed.starts_with("using "),
        "cpp" | "cc" | "cxx" | "c" | "h" | "hpp" => trimmed.starts_with("#include"),
        "php" => trimmed.starts_with("use ") || trimmed.starts_with("require"),
        "rb" => trimmed.starts_with("require"),
        _ => false,
    }
}

/// Find the nearest definition (fn/class/struct/def/func) at or above the cursor line.
fn nearest_definition(lines: &[&str], cursor_line: usize, ext: &str) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    // Walk backwards from cursor_line.
    for i in (0..=cursor_line.min(lines.len().saturating_sub(1))).rev() {
        let line = lines[i].trim();
        if is_definition_line(line, ext) {
            return Some(lines[i].to_string());
        }
    }
    None
}

/// Return true if the line looks like a function/class/struct definition.
fn is_definition_line(line: &str, ext: &str) -> bool {
    match ext {
        "rs" => {
            line.starts_with("pub fn ")
                || line.starts_with("fn ")
                || line.starts_with("pub struct ")
                || line.starts_with("struct ")
                || line.starts_with("pub enum ")
                || line.starts_with("enum ")
                || line.starts_with("impl ")
                || line.starts_with("pub impl ")
                || line.starts_with("trait ")
                || line.starts_with("pub trait ")
        }
        "ts" | "tsx" | "js" | "jsx" => {
            line.starts_with("function ")
                || line.starts_with("async function ")
                || line.starts_with("export function ")
                || line.starts_with("export async function ")
                || line.starts_with("class ")
                || line.starts_with("export class ")
                || line.starts_with("export default ")
                || (line.starts_with("const ") && line.contains("=>"))
        }
        "dart" => {
            line.starts_with("class ")
                || line.starts_with("abstract class ")
                || line.starts_with("mixin ")
                || line.starts_with("extension ")
                || (line.contains('(') && !line.starts_with("//") && !line.starts_with("*"))
        }
        "py" | "pyw" => {
            line.starts_with("def ") || line.starts_with("async def ") || line.starts_with("class ")
        }
        "go" => line.starts_with("func ") || line.starts_with("type "),
        "java" | "kt" | "kts" => {
            line.contains("class ")
                || line.contains("fun ")
                || line.contains("void ")
                || line.contains("interface ")
        }
        _ => false,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_extraction_rust() {
        let content = "use std::io;\nuse anyhow::Result;\n\nfn main() {\n    let x = 1;\n}";
        let ctx = extract_context(content, 4, "main.rs");
        assert!(
            ctx.contains("use std::io;"),
            "should include use statements"
        );
        assert!(ctx.contains("use anyhow::Result;"));
    }

    #[test]
    fn enclosing_fn_extracted() {
        let content = "fn compute(x: i32) -> i32 {\n    let y = x + 1;\n    y\n}";
        let ctx = extract_context(content, 2, "lib.rs");
        assert!(
            ctx.contains("fn compute"),
            "should include nearest fn signature"
        );
    }

    #[test]
    fn no_context_for_empty_file() {
        let ctx = extract_context("", 0, "file.rs");
        assert!(ctx.is_empty(), "empty file should yield empty context");
    }

    #[test]
    fn context_capped_at_max_chars() {
        // Create a file with many import lines exceeding 2048 chars.
        let imports: String = (0..200).map(|i| format!("use module_{i};\n")).collect();
        let ctx = extract_context(&imports, 0, "big.rs");
        // Strip the framing to just the content.
        let content_len = ctx.len();
        assert!(
            content_len <= MAX_CONTEXT_CHARS + 64, // +64 for framing text
            "context should be capped; got {content_len} chars"
        );
    }

    #[test]
    fn dart_import_detection() {
        let content =
            "import 'dart:io';\nimport 'package:flutter/material.dart';\n\nvoid main() {}";
        let ctx = extract_context(content, 3, "main.dart");
        assert!(ctx.contains("import 'dart:io';"));
    }
}
