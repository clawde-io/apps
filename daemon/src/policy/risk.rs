//! Tool risk classification — maps tool names to `RiskLevel`.
//!
//! `RiskDatabase` is loaded once at daemon start from
//! `.claw/policies/tool-risk.json` (if it exists) and falls back to
//! `RiskDatabase::default_rules()` for any unknown tool.
//!
//! **Note:** `RiskLevel` is re-exported from `crate::tasks::schema` so the
//! same canonical type is shared across the entire codebase. This module does
//! NOT define a second `RiskLevel` enum.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

// Re-export the canonical RiskLevel from tasks::schema.
pub use crate::tasks::schema::RiskLevel;

// ─── Risk database ────────────────────────────────────────────────────────────

/// Maps tool names to their assigned risk level.
///
/// Loaded from `.claw/policies/tool-risk.json`; falls back to
/// `default_rules()` for any tool not present in the file.
#[derive(Debug, Clone, Default)]
pub struct RiskDatabase {
    rules: HashMap<String, RiskLevel>,
}

/// JSON shape expected in `tool-risk.json`.
#[derive(Debug, Deserialize)]
struct RiskConfigFile {
    #[serde(default)]
    low: Vec<String>,
    #[serde(default)]
    medium: Vec<String>,
    #[serde(default)]
    high: Vec<String>,
    #[serde(default)]
    critical: Vec<String>,
}

impl RiskDatabase {
    /// Hardcoded default risk rules for all built-in clawd tools.
    pub fn default_rules() -> Self {
        let mut rules = HashMap::new();

        // ── Low risk — read-only, non-destructive ─────────────────────────
        for tool in &["read_file", "search_files", "log_event"] {
            rules.insert((*tool).to_string(), RiskLevel::Low);
        }

        // ── Medium risk — state-mutating but reversible ───────────────────
        for tool in &[
            "run_tests",
            "create_task",
            "claim_task",
            "transition_task",
        ] {
            rules.insert((*tool).to_string(), RiskLevel::Medium);
        }

        // ── High risk — file mutations and approval gating ────────────────
        for tool in &["apply_patch", "request_approval"] {
            rules.insert((*tool).to_string(), RiskLevel::High);
        }

        // ── Critical risk — network, push, and shell execution ────────────
        for tool in &["git_push", "shell_exec", "network_request"] {
            rules.insert((*tool).to_string(), RiskLevel::Critical);
        }

        Self { rules }
    }

    /// Load a risk database from a JSON file.
    ///
    /// Missing or malformed files emit a warning and return the default rules.
    pub fn load_from_json(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!(path = %path.display(), err = %e, "tool-risk.json not found — using defaults");
                return Self::default_rules();
            }
        };

        let config: RiskConfigFile = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                warn!(err = %e, "tool-risk.json parse error — using defaults");
                return Self::default_rules();
            }
        };

        // Start from defaults so unknown tools fall back correctly.
        let mut db = Self::default_rules();

        for tool in config.low {
            db.rules.insert(tool, RiskLevel::Low);
        }
        for tool in config.medium {
            db.rules.insert(tool, RiskLevel::Medium);
        }
        for tool in config.high {
            db.rules.insert(tool, RiskLevel::High);
        }
        for tool in config.critical {
            db.rules.insert(tool, RiskLevel::Critical);
        }

        db
    }

    /// Return the risk level for the given tool name.
    ///
    /// Defaults to `Medium` when the tool is not in the database, so unknown
    /// tools require an active task before they can execute.
    pub fn get_risk(&self, tool: &str) -> RiskLevel {
        self.rules
            .get(tool)
            .cloned()
            .unwrap_or(RiskLevel::Medium)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rules_low_risk() {
        let db = RiskDatabase::default_rules();
        assert_eq!(db.get_risk("read_file"), RiskLevel::Low);
        assert_eq!(db.get_risk("log_event"), RiskLevel::Low);
    }

    #[test]
    fn default_rules_medium_risk() {
        let db = RiskDatabase::default_rules();
        assert_eq!(db.get_risk("run_tests"), RiskLevel::Medium);
        assert_eq!(db.get_risk("create_task"), RiskLevel::Medium);
    }

    #[test]
    fn default_rules_high_risk() {
        let db = RiskDatabase::default_rules();
        assert_eq!(db.get_risk("apply_patch"), RiskLevel::High);
        assert_eq!(db.get_risk("request_approval"), RiskLevel::High);
    }

    #[test]
    fn default_rules_critical() {
        let db = RiskDatabase::default_rules();
        assert_eq!(db.get_risk("git_push"), RiskLevel::Critical);
        assert_eq!(db.get_risk("shell_exec"), RiskLevel::Critical);
    }

    #[test]
    fn unknown_tool_defaults_to_medium() {
        let db = RiskDatabase::default_rules();
        assert_eq!(db.get_risk("some_new_tool_xyz"), RiskLevel::Medium);
    }
}
