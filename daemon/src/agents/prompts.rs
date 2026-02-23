//! Prompt version tracking for agent templates (Phase 43e / 43k-8).
//!
//! Tracks SHA-256 hashes of agent prompt templates. When a prompt changes
//! the eval pipeline (Phase 43h) should be triggered to validate the new
//! prompt against the acceptance test suite.

use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Stores version hashes of agent prompt templates.
///
/// A change in hash signals that the prompt has been updated and evals
/// should re-run before the new prompt is deployed to production agents.
pub struct PromptVersionStore {
    /// Prompt name → SHA-256 hex digest.
    versions: HashMap<String, String>,
}

impl PromptVersionStore {
    pub fn new() -> Self {
        Self {
            versions: HashMap::new(),
        }
    }

    /// Compute a SHA-256 hex digest of prompt content.
    pub fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Register (or update) a prompt. Returns `true` if the content changed.
    pub fn register(&mut self, name: &str, content: &str) -> bool {
        let hash = Self::hash_content(content);
        let changed = self
            .versions
            .get(name)
            .map_or(true, |existing| existing != &hash);
        self.versions.insert(name.to_string(), hash);
        changed
    }

    /// Get the stored hash for a named prompt.
    pub fn get_hash(&self, name: &str) -> Option<&str> {
        self.versions.get(name).map(|s| s.as_str())
    }

    /// Return all stored name → hash pairs.
    pub fn all_versions(&self) -> &HashMap<String, String> {
        &self.versions
    }
}

impl Default for PromptVersionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_changed_prompt() {
        let mut store = PromptVersionStore::new();
        let changed_first = store.register("router", "You are the Router agent.");
        assert!(changed_first, "first registration should be marked as changed");

        let changed_same = store.register("router", "You are the Router agent.");
        assert!(!changed_same, "same content should not be marked as changed");

        let changed_new = store.register("router", "You are the Router agent v2.");
        assert!(changed_new, "different content should be marked as changed");
    }

    #[test]
    fn hash_is_deterministic() {
        let h1 = PromptVersionStore::hash_content("hello");
        let h2 = PromptVersionStore::hash_content("hello");
        assert_eq!(h1, h2);
    }
}
