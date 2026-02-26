// agents/copilot.rs — GitHub Copilot CLI driver (Sprint ZZ MP.T01)
//
// Adapter for `gh copilot suggest` / `gh copilot explain` commands.
// provider: "copilot"
// Capabilities: suggest, explain (not full session management)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Stdio;

/// Capability marker for the Copilot CLI provider.
pub const PROVIDER_NAME: &str = "copilot";

/// Check if the `gh copilot` CLI extension is installed and available.
pub async fn is_available() -> bool {
    tokio::process::Command::new("gh")
        .args(["copilot", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get the installed version of the gh copilot extension.
pub async fn version() -> Option<String> {
    let output = tokio::process::Command::new("gh")
        .args(["copilot", "--version"])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Some(stdout.trim().to_string())
    } else {
        None
    }
}

/// Request type for Copilot CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CopilotRequest {
    /// `gh copilot suggest` — generate a shell command suggestion.
    Suggest {
        prompt: String,
        target: SuggestTarget,
    },
    /// `gh copilot explain` — explain a shell command.
    Explain { command: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestTarget {
    /// Git commands
    Git,
    /// GitHub CLI commands
    Gh,
    /// General shell commands
    Shell,
}

impl SuggestTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Gh => "gh",
            Self::Shell => "shell",
        }
    }
}

/// Response from the Copilot CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotResponse {
    pub content: String,
    pub provider: &'static str,
}

/// Run `gh copilot suggest` with a prompt.
pub async fn suggest(prompt: &str, target: &SuggestTarget) -> Result<CopilotResponse> {
    let output = tokio::process::Command::new("gh")
        .args(["copilot", "suggest", "-t", target.as_str(), "--", prompt])
        .output()
        .await
        .context("Failed to run gh copilot suggest")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh copilot suggest failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(CopilotResponse {
        content: stdout.trim().to_string(),
        provider: PROVIDER_NAME,
    })
}

/// Run `gh copilot explain` with a command.
pub async fn explain(command: &str) -> Result<CopilotResponse> {
    let output = tokio::process::Command::new("gh")
        .args(["copilot", "explain", "--", command])
        .output()
        .await
        .context("Failed to run gh copilot explain")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh copilot explain failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(CopilotResponse {
        content: stdout.trim().to_string(),
        provider: PROVIDER_NAME,
    })
}

/// Build the capability matrix JSON for this provider.
pub fn capability_matrix() -> serde_json::Value {
    serde_json::json!({
        "full_session": false,
        "code_generation": true,
        "explain": true,
        "code_review": false,
        "lsp_integration": false,
        "arena_mode": false,
        "suggest": true,
    })
}
