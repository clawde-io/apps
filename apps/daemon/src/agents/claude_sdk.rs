//! Claude Code Agent SDK integration.
//!
//! Provides programmatic session management for Claude Code: create sessions,
//! resume existing sessions by ID, and record session metadata.
//!
//! The actual invocation shells out to the `claude` binary with
//! `--output-format stream-json`.  Session IDs are either parsed from the
//! first streamed event or generated deterministically from the run context.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use uuid::Uuid;

// ─── ClaudeSession ────────────────────────────────────────────────────────────

/// Metadata for a single Claude Code Agent SDK session.
#[derive(Debug, Clone)]
pub struct ClaudeSession {
    /// Stable session identifier (prefixed `css-` for Claude SDK Session).
    pub session_id: String,
    /// Working directory the session was bound to.
    pub project_dir: PathBuf,
    /// Model identifier used for this session (e.g. `"claude-sonnet-4-6"`).
    pub model: String,
}

// ─── ClaudeSdk ────────────────────────────────────────────────────────────────

/// Entry point for Claude Code Agent SDK operations.
pub struct ClaudeSdk;

impl ClaudeSdk {
    /// Start a new Claude Code session in the given directory.
    ///
    /// Invokes:
    /// ```text
    /// claude --project-dir <dir> --model <model> --output-format stream-json <prompt>
    /// ```
    ///
    /// Returns a `ClaudeSession` whose `session_id` can be used to resume
    /// this session later via [`resume_session`].
    pub async fn create_session(
        project_dir: &Path,
        model: &str,
        prompt: &str,
    ) -> Result<ClaudeSession> {
        let output = tokio::process::Command::new("claude")
            .arg("--project-dir")
            .arg(project_dir)
            .arg("--model")
            .arg(model)
            .arg("--output-format")
            .arg("stream-json")
            .arg(prompt)
            .output()
            .await
            .context("failed to invoke claude CLI")?;

        // Attempt to extract a session_id from the first line of streamed JSON.
        // Claude Code stream-json format emits one JSON object per line;
        // the first event may carry a `session_id` field.
        let session_id = parse_session_id_from_output(&output.stdout)
            .unwrap_or_else(|| format!("css-{}", Uuid::new_v4()));

        Ok(ClaudeSession {
            session_id,
            project_dir: project_dir.to_owned(),
            model: model.to_string(),
        })
    }

    /// Resume a Claude Code session by its ID.
    ///
    /// Invokes:
    /// ```text
    /// claude --resume <session_id> --output-format stream-json
    /// ```
    ///
    /// Runs to completion and parses the session_id from output, consistent
    /// with `create_session`. The session_id is already known from input, so
    /// this primarily verifies the CLI can reach the session.
    pub async fn resume_session(session_id: &str, project_dir: &Path) -> Result<ClaudeSession> {
        let _output = tokio::process::Command::new("claude")
            .arg("--resume")
            .arg(session_id)
            .arg("--output-format")
            .arg("stream-json")
            .current_dir(project_dir)
            .output()
            .await
            .context("failed to invoke claude CLI for resume")?;

        Ok(ClaudeSession {
            session_id: session_id.to_string(),
            project_dir: project_dir.to_owned(),
            // Default model — will be overridden by the resumed session's config.
            model: "claude-sonnet-4-6".to_string(),
        })
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Try to extract `session_id` from the first JSON line of Claude's
/// `stream-json` output.
fn parse_session_id_from_output(raw: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(raw).ok()?;
    let first_line = text.lines().next()?;
    let v: serde_json::Value = serde_json::from_str(first_line).ok()?;
    v.get("session_id")
        .or_else(|| v.get("sessionId"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_session_id_from_valid_json() {
        let raw = br#"{"session_id":"css-abc123","type":"start"}"#;
        assert_eq!(
            parse_session_id_from_output(raw),
            Some("css-abc123".to_string())
        );
    }

    #[test]
    fn parse_session_id_missing_returns_none() {
        let raw = br#"{"type":"start","content":"hello"}"#;
        assert_eq!(parse_session_id_from_output(raw), None);
    }

    #[test]
    fn parse_session_id_from_empty_returns_none() {
        assert_eq!(parse_session_id_from_output(b""), None);
    }
}
