//! Prompt cache key computation for stable system-prompt prefixes (Sprint BB PV.9).
//!
//! The `stable_prefix_hash` is a SHA-256 digest of the components that make up
//! the stable (cacheable) portion of a session's system prompt:
//!
//!   `SHA-256(system_prompt || sorted_repo_context_paths || repo_HEAD_sha)`
//!
//! The hash changes when:
//! - The system prompt text changes (coding standards update, etc.)
//! - The set of repo context file paths changes (new files added/removed)
//! - A new commit lands on the repo HEAD
//!
//! It does NOT change between turns when none of the above have changed, which
//! means the Anthropic and OpenAI prompt caches remain valid across turns.

use sha2::{Digest, Sha256};

// ─── Hash computation ─────────────────────────────────────────────────────────

/// Compute a stable cache key for a session's system prompt prefix.
///
/// # Arguments
/// * `system_prompt` — the full system prompt string (standards + provider knowledge).
/// * `repo_context_paths` — all file paths included in the repo context
///   (will be sorted before hashing to ensure determinism).
/// * `repo_head_sha` — the current HEAD commit SHA of the repository.
///   Use an empty string if the session has no associated repo.
///
/// # Returns
/// A 64-character lowercase hex string (SHA-256).
pub fn stable_prefix_hash(
    system_prompt: &str,
    repo_context_paths: &[&str],
    repo_head_sha: &str,
) -> String {
    let mut hasher = Sha256::new();

    // 1. System prompt (most stable — rarely changes mid-project).
    hasher.update(system_prompt.as_bytes());
    hasher.update(b"\x00"); // NUL separator

    // 2. Sorted repo context file paths (order must be deterministic).
    let mut sorted_paths = repo_context_paths.to_vec();
    sorted_paths.sort_unstable();
    for path in &sorted_paths {
        hasher.update(path.as_bytes());
        hasher.update(b"\x01"); // SOH separator between paths
    }
    hasher.update(b"\x00"); // NUL separator after paths block

    // 3. Repository HEAD SHA (changes on every commit).
    hasher.update(repo_head_sha.as_bytes());

    let result = hasher.finalize();
    hex::encode(result)
}

/// True if two hashes differ, meaning the cached prompt prefix is stale.
pub fn prefix_changed(old_hash: &str, new_hash: &str) -> bool {
    old_hash != new_hash
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const PROMPT: &str = "You are an expert Rust engineer.";
    const HEAD: &str = "abc123def456";

    #[test]
    fn same_inputs_produce_same_hash() {
        let paths = vec!["src/main.rs", "src/lib.rs"];
        let h1 = stable_prefix_hash(PROMPT, &paths, HEAD);
        let h2 = stable_prefix_hash(PROMPT, &paths, HEAD);
        assert_eq!(h1, h2);
    }

    #[test]
    fn path_order_does_not_matter() {
        let paths_a = vec!["src/main.rs", "src/lib.rs"];
        let paths_b = vec!["src/lib.rs", "src/main.rs"];
        assert_eq!(
            stable_prefix_hash(PROMPT, &paths_a, HEAD),
            stable_prefix_hash(PROMPT, &paths_b, HEAD),
        );
    }

    #[test]
    fn different_head_sha_changes_hash() {
        let paths = vec!["src/main.rs"];
        let h1 = stable_prefix_hash(PROMPT, &paths, HEAD);
        let h2 = stable_prefix_hash(PROMPT, &paths, "deadbeef");
        assert_ne!(h1, h2);
    }

    #[test]
    fn different_system_prompt_changes_hash() {
        let paths = vec!["src/main.rs"];
        let h1 = stable_prefix_hash(PROMPT, &paths, HEAD);
        let h2 = stable_prefix_hash("You are an expert TypeScript engineer.", &paths, HEAD);
        assert_ne!(h1, h2);
    }

    #[test]
    fn added_path_changes_hash() {
        let paths_a = vec!["src/main.rs"];
        let paths_b = vec!["src/main.rs", "src/lib.rs"];
        assert_ne!(
            stable_prefix_hash(PROMPT, &paths_a, HEAD),
            stable_prefix_hash(PROMPT, &paths_b, HEAD),
        );
    }

    #[test]
    fn empty_inputs_produce_valid_hash() {
        let hash = stable_prefix_hash("", &[], "");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn prefix_changed_detects_mismatch() {
        let paths = vec!["src/main.rs"];
        let old = stable_prefix_hash(PROMPT, &paths, HEAD);
        let new = stable_prefix_hash(PROMPT, &paths, "newhead");
        assert!(prefix_changed(&old, &new));
        assert!(!prefix_changed(&old, &old));
    }
}
