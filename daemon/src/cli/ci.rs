//! Sprint EE CI.9 — `clawd ci` CLI commands.
//!
//! Non-interactive CI runner for use in GitHub Actions and other CI environments.
//!
//! ## Usage
//!
//! ```text
//! clawd ci run [--repo <path>] [--step <name>]
//! clawd ci status <run-id>
//! ```

use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};

/// `clawd ci run` — execute the CI config for the repo.
pub async fn run(
    repo_path: Option<PathBuf>,
    step: Option<String>,
    data_dir: &Path,
    port: u16,
) -> Result<()> {
    let repo = repo_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    println!("Starting CI run in {repo} ...");

    let mut params = json!({ "repoPath": repo });
    if let Some(s) = step {
        params["step"] = serde_json::Value::String(s);
    }

    let result = client.call_once("ci.run", params).await?;
    let run_id = result.get("runId").and_then(|v| v.as_str()).unwrap_or("?");

    println!("CI run started — runId: {run_id}");
    println!("Watching for completion... (Ctrl+C to cancel)");

    // Poll status until complete
    let poll_client = super::client::DaemonClient::new(
        port,
        super::client::read_auth_token(data_dir)?,
    );
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let status_result = poll_client
            .call_once("ci.status", json!({ "runId": run_id }))
            .await?;
        let status = status_result
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match status {
            "success" => {
                println!("CI run succeeded.");
                std::process::exit(0);
            }
            "failure" => {
                eprintln!("CI run failed.");
                std::process::exit(1);
            }
            "canceled" => {
                println!("CI run was canceled.");
                std::process::exit(1);
            }
            _ => {
                print!(".");
            }
        }
    }
}

/// `clawd ci status <run-id>` — print status of a CI run.
pub async fn status(run_id: String, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client
        .call_once("ci.status", json!({ "runId": run_id }))
        .await?;
    let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("?");

    println!("Run {run_id}: {status}");
    Ok(())
}
