// agents/gemini.rs — Google Gemini CLI driver (Sprint ZZ MP.T02)
//
// Adapter for the `gemini` command-line tool.
// provider: "gemini"
// Full session management if CLI supports it; explain + suggest mode if not.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Stdio;

pub const PROVIDER_NAME: &str = "gemini";

/// Check if the `gemini` CLI is installed and available.
pub async fn is_available() -> bool {
    tokio::process::Command::new("gemini")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get the installed version of the Gemini CLI.
pub async fn version() -> Option<String> {
    let output = tokio::process::Command::new("gemini")
        .arg("--version")
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

/// Response from the Gemini CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub content: String,
    pub provider: &'static str,
    pub model: Option<String>,
}

/// Send a single non-interactive prompt to Gemini CLI.
///
/// Maps to: `gemini -p "<prompt>"`
pub async fn prompt(text: &str) -> Result<GeminiResponse> {
    let output = tokio::process::Command::new("gemini")
        .args(["-p", text])
        .output()
        .await
        .context("Failed to run gemini CLI")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gemini CLI failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(GeminiResponse {
        content: stdout.trim().to_string(),
        provider: PROVIDER_NAME,
        model: None,
    })
}

/// Check if the Gemini CLI supports interactive sessions.
///
/// Detects by looking for `--session` or `--continue` flags in help output.
pub async fn supports_sessions() -> bool {
    let output = tokio::process::Command::new("gemini")
        .arg("--help")
        .output()
        .await;

    match output {
        Ok(o) => {
            let help = String::from_utf8_lossy(&o.stdout);
            let help_err = String::from_utf8_lossy(&o.stderr);
            let combined = format!("{help}{help_err}");
            combined.contains("--session") || combined.contains("--continue")
        }
        Err(_) => false,
    }
}

/// Build the capability matrix JSON for this provider.
pub async fn capability_matrix() -> serde_json::Value {
    let session_support = supports_sessions().await;
    serde_json::json!({
        "full_session": session_support,
        "code_generation": true,
        "explain": true,
        "code_review": true,
        "lsp_integration": false,
        "arena_mode": false,
        "suggest": true,
    })
}
