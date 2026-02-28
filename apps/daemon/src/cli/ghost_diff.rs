//! Sprint CC GD.6 — `clawd ghost-diff` CLI command.
//!
//! Runs a ghost diff check against the daemon for the current repo and prints
//! any spec drift warnings.
//!
//! ## Usage
//!
//! ```text
//! clawd ghost-diff [--repo <path>] [--session <id>]
//! ```
//!
//! Requires the daemon to be running. Reads the auth token from the standard
//! token file location.

use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;

/// Run the ghost-diff check by calling the daemon RPC and printing results.
///
/// `data_dir` is passed in from the CLI config so we don't need to re-derive it.
pub async fn run(
    repo_path: Option<PathBuf>,
    session_id: Option<String>,
    data_dir: &std::path::Path,
    port: u16,
) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let repo = repo_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    let params = json!({
        "repoPath": repo,
        "sessionId": session_id,
    });

    let result = client.call_once("ghost_diff.check", params).await?;

    let has_drift = result
        .get("hasDrift")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !has_drift {
        println!("No spec drift detected in {}", repo);
        return Ok(());
    }

    let warnings = result
        .get("warnings")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    println!(
        "{} spec drift warning(s) detected in {}:\n",
        warnings.len(),
        repo
    );

    for (i, w) in warnings.iter().enumerate() {
        let file = w.get("file").and_then(|v| v.as_str()).unwrap_or("?");
        let spec = w.get("spec").and_then(|v| v.as_str()).unwrap_or("?");
        let summary = w
            .get("divergenceSummary")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let severity = w.get("severity").and_then(|v| v.as_str()).unwrap_or("low");

        println!(
            "  {}. [{}] {} (spec: {})",
            i + 1,
            severity.to_uppercase(),
            file,
            spec
        );
        if !summary.is_empty() {
            println!("     {}", summary);
        }
    }

    Ok(())
}
