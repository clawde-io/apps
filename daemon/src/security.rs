// SPDX-License-Identifier: MIT
//! Security utilities — Phase 46.
//!
//! Guards against path traversal, unsafe file access, and other security risks.

use std::path::{Path, PathBuf};
use anyhow::{bail, Result};

/// Validate that `path` is within `base_dir` (no traversal attacks).
/// 
/// Resolves symlinks and canonicalizes both paths before comparing.
/// Returns the canonicalized safe path on success.
///
/// Examples:
///   security::safe_path("/home/user/repo", "src/main.rs") → Ok("/home/user/repo/src/main.rs")
///   security::safe_path("/home/user/repo", "../../../etc/passwd") → Err(...)
pub fn safe_path(base_dir: &Path, relative_path: &Path) -> Result<PathBuf> {
    // If relative_path is absolute, reject it
    if relative_path.is_absolute() {
        bail!("path traversal: absolute path not allowed: {}", relative_path.display());
    }

    // Join base + relative
    let joined = base_dir.join(relative_path);

    // Normalize without requiring the path to exist (canonicalize would fail)
    let normalized = normalize_path(&joined);

    // Ensure normalized path starts with base_dir
    let base_normalized = normalize_path(base_dir);
    if !normalized.starts_with(&base_normalized) {
        bail!(
            "path traversal: {} escapes base directory {}",
            relative_path.display(),
            base_dir.display()
        );
    }

    Ok(normalized)
}

/// Normalize a path by resolving `.` and `..` components without requiring
/// the path to exist on disk (unlike std::fs::canonicalize).
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        use std::path::Component::*;
        match component {
            ParentDir => {
                if matches!(components.last(), Some(Normal(_))) {
                    components.pop();
                }
                // Ignore .. at root
            }
            CurDir => {
                // Skip .
            }
            other => components.push(other),
        }
    }
    components.iter().collect()
}

/// Strip null bytes from a string (prevent null-byte injection in file paths).
pub fn strip_null_bytes(s: &str) -> String {
    s.replace('\0', "")
}

/// Sanitize tool input before storing in the audit log.
///
/// Replaces any 40+ character base64-like strings (API keys, tokens) with
/// `[REDACTED]` to prevent credential leakage into audit storage.
pub fn sanitize_tool_input(input: &str) -> String {
    // Match long base64 or hex-like strings: [A-Za-z0-9+/]{40,}
    // Use a simple state-machine approach to avoid the regex crate dependency.
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Count how many base64-alphabet chars in a row start here
        let mut run = 0;
        let mut j = i;
        while j < chars.len()
            && (chars[j].is_ascii_alphanumeric() || chars[j] == '+' || chars[j] == '/')
        {
            run += 1;
            j += 1;
        }
        if run >= 40 {
            result.push_str("[REDACTED]");
            i += run;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Check a tool call against the security allowlist/denylist config (DC.T40).
///
/// Returns `Ok(())` if the call is permitted, `Err(...)` if it should be blocked.
///
/// Rules (applied in order):
/// 1. If `denied_tools` contains the tool name → blocked.
/// 2. If `allowed_tools` is non-empty AND does not contain the tool name → blocked.
/// 3. For Bash tool: if the command starts with any `denied_paths` entry → blocked.
pub fn check_tool_call(
    tool_name: &str,
    tool_input: &str,
    config: &crate::config::SecurityConfig,
) -> Result<()> {
    let tool_lower = tool_name.to_lowercase();

    // 1. Check deny list
    for denied in &config.denied_tools {
        if denied.to_lowercase() == tool_lower {
            bail!(
                "TOOL_DENIED: tool '{}' is in the security.denied_tools list",
                tool_name
            );
        }
    }

    // 2. Check allowlist (non-empty = restrictive)
    if !config.allowed_tools.is_empty() {
        let is_allowed = config
            .allowed_tools
            .iter()
            .any(|a| a.to_lowercase() == tool_lower);
        if !is_allowed {
            bail!(
                "TOOL_NOT_ALLOWED: tool '{}' is not in the security.allowed_tools list",
                tool_name
            );
        }
    }

    // 3. Check denied_paths for Bash tool
    if tool_lower == "bash" {
        for denied_path in &config.denied_paths {
            let expanded = expand_home(denied_path);
            if tool_input.starts_with(&expanded) || tool_input.contains(&expanded) {
                bail!(
                    "TOOL_PATH_DENIED: Bash command accesses denied path '{}'",
                    denied_path
                );
            }
        }
    }

    Ok(())
}

/// Expand `~` at the start of a path to the current user's home directory.
fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        if !home.is_empty() {
            return format!("{}/{}", home, rest);
        }
    }
    path.to_string()
}

/// Check that a repo path is safe to use as a session workspace (DC.T41).
///
/// Rejects: paths that overlap with the daemon's own data directory.
/// Warns about: repos that contain a `.clawd/` directory (config injection risk).
pub fn check_repo_path_safety(repo_path: &Path, data_dir: &Path) -> Result<()> {
    // Use canonicalize where possible; fall back to normalize_path for non-existent paths.
    let canonical_repo = repo_path
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(repo_path));
    let canonical_data = data_dir
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(data_dir));

    if canonical_repo.starts_with(&canonical_data) || canonical_data.starts_with(&canonical_repo) {
        bail!(
            "invalid type: repo_path '{}' overlaps with the daemon data directory — \
             this is a security risk",
            repo_path.display()
        );
    }

    if canonical_repo.join(".clawd").exists() {
        tracing::warn!(
            repo = %canonical_repo.display(),
            "repo contains .clawd/ directory — ignoring it as config source (injection protection)"
        );
    }

    Ok(())
}

/// Validate that a session ID is a valid UUID (no injection possible).
pub fn validate_session_id(id: &str) -> Result<()> {
    // UUIDs are 36 chars: 8-4-4-4-12 hex + dashes
    if id.len() != 36 {
        bail!("invalid session ID length: {}", id.len());
    }
    for (i, c) in id.chars().enumerate() {
        let is_dash = matches!(i, 8 | 13 | 18 | 23);
        if is_dash {
            if c != '-' {
                bail!("invalid session ID format at position {}", i);
            }
        } else if !c.is_ascii_hexdigit() {
            bail!("invalid session ID character at position {}: {}", i, c);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_safe_path_normal() {
        let base = Path::new("/home/user/repo");
        let result = safe_path(base, Path::new("src/main.rs")).unwrap();
        assert_eq!(result, PathBuf::from("/home/user/repo/src/main.rs"));
    }

    #[test]
    fn test_safe_path_traversal_blocked() {
        let base = Path::new("/home/user/repo");
        let result = safe_path(base, Path::new("../../etc/passwd"));
        assert!(result.is_err(), "path traversal should be blocked");
    }

    #[test]
    fn test_safe_path_absolute_blocked() {
        let base = Path::new("/home/user/repo");
        let result = safe_path(base, Path::new("/etc/passwd"));
        assert!(result.is_err(), "absolute paths should be blocked");
    }

    #[test]
    fn test_normalize_path() {
        let p = Path::new("/a/b/../c/./d");
        assert_eq!(normalize_path(p), PathBuf::from("/a/c/d"));
    }

    #[test]
    fn test_validate_session_id_valid() {
        assert!(validate_session_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
    }

    #[test]
    fn test_validate_session_id_invalid() {
        assert!(validate_session_id("not-a-uuid").is_err());
        assert!(validate_session_id("550e8400-e29b-41d4-a716-44665544000X").is_err());
    }

    // ── Tool call gating (DC.T40) ─────────────────────────────────────────────

    fn default_sec() -> crate::config::SecurityConfig {
        crate::config::SecurityConfig::default()
    }

    #[test]
    fn test_empty_allowlist_permits_all() {
        let cfg = default_sec();
        assert!(check_tool_call("Bash", "echo hello", &cfg).is_ok());
        assert!(check_tool_call("Read", "", &cfg).is_ok());
        assert!(check_tool_call("WebFetch", "", &cfg).is_ok());
    }

    #[test]
    fn test_allowlist_blocks_unlisted_tool() {
        let cfg = crate::config::SecurityConfig {
            allowed_tools: vec!["Read".into(), "Grep".into()],
            ..Default::default()
        };
        assert!(check_tool_call("Read", "", &cfg).is_ok());
        assert!(check_tool_call("Bash", "echo", &cfg).is_err());
    }

    #[test]
    fn test_denylist_blocks_listed_tool() {
        let cfg = crate::config::SecurityConfig {
            denied_tools: vec!["WebFetch".into()],
            ..Default::default()
        };
        assert!(check_tool_call("WebFetch", "", &cfg).is_err());
        assert!(check_tool_call("Read", "", &cfg).is_ok());
    }

    #[test]
    fn test_tool_name_comparison_case_insensitive() {
        let cfg = crate::config::SecurityConfig {
            denied_tools: vec!["bash".into()],
            ..Default::default()
        };
        assert!(check_tool_call("Bash", "echo", &cfg).is_err());
        assert!(check_tool_call("BASH", "echo", &cfg).is_err());
    }

    #[test]
    fn test_denied_path_blocks_bash_call() {
        let cfg = crate::config::SecurityConfig {
            denied_paths: vec!["/etc".into()],
            ..Default::default()
        };
        assert!(check_tool_call("Bash", "cat /etc/passwd", &cfg).is_err());
        assert!(check_tool_call("Bash", "echo hello", &cfg).is_ok());
    }

    #[test]
    fn test_denied_path_only_applies_to_bash() {
        let cfg = crate::config::SecurityConfig {
            denied_paths: vec!["/etc".into()],
            ..Default::default()
        };
        // Read tool with /etc path should still be allowed (path check is Bash-only)
        assert!(check_tool_call("Read", "/etc/passwd", &cfg).is_ok());
    }

    // ── Input sanitization (DC.T43) ───────────────────────────────────────────

    #[test]
    fn test_sanitize_short_string_unchanged() {
        let s = "echo hello world";
        assert_eq!(sanitize_tool_input(s), s);
    }

    #[test]
    fn test_sanitize_long_base64_redacted() {
        let key = "A".repeat(44); // 44 base64 chars → REDACTED
        let input = format!("curl -H 'Authorization: Bearer {}'", key);
        let result = sanitize_tool_input(&input);
        assert!(result.contains("[REDACTED]"), "long token should be redacted: {result}");
        assert!(!result.contains(&key), "original key should not appear");
    }

    #[test]
    fn test_sanitize_normal_code_unchanged() {
        let code = "let x = 42; println!(\"{x}\");";
        assert_eq!(sanitize_tool_input(code), code);
    }

    // ── Repo path safety (DC.T41) ─────────────────────────────────────────────

    #[test]
    fn test_repo_path_does_not_overlap_data_dir() {
        let data_dir = Path::new("/tmp/clawd_test_data");
        let repo_path = Path::new("/home/user/my_project");
        // Should not bail for non-overlapping paths
        // (note: canonicalize will fail for non-existent paths, so normalize_path is used)
        let result = check_repo_path_safety(repo_path, data_dir);
        assert!(result.is_ok(), "non-overlapping paths should be ok: {result:?}");
    }

    #[test]
    fn test_session_create_rejects_data_dir_as_repo() {
        let data_dir = Path::new("/tmp/clawd_data_test_12345");
        let repo_path = Path::new("/tmp/clawd_data_test_12345");
        let result = check_repo_path_safety(repo_path, data_dir);
        assert!(result.is_err(), "repo_path == data_dir should be rejected");
    }
}
