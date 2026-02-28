// SPDX-License-Identifier: MIT
//! VS Code compatibility bridge — Sprint S (LS.T05, LS.T09)
//!
//! Provides:
//! - [`extension_host`] — detect and list VS Code extensions for a workspace
//! - [`settings_bridge`] — read/write VS Code user and workspace settings

pub mod extension_host;
pub mod settings_bridge;

/// Strip `//` and `/* */` comments from JSONC (JSON with comments) content.
///
/// VS Code settings files are JSONC; standard `serde_json` cannot parse them directly.
/// This removes comments so the result is valid JSON.
pub fn strip_jsonc_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            result.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    result.push(next);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => {
                in_string = true;
                result.push(c);
            }
            '/' => match chars.peek() {
                Some('/') => {
                    chars.next();
                    for nc in chars.by_ref() {
                        if nc == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    chars.next();
                    let mut prev = ' ';
                    for nc in chars.by_ref() {
                        if prev == '*' && nc == '/' {
                            break;
                        }
                        if nc == '\n' {
                            result.push('\n');
                        }
                        prev = nc;
                    }
                }
                _ => result.push(c),
            },
            _ => result.push(c),
        }
    }

    result
}
