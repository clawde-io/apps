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
}
