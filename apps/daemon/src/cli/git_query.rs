//! Sprint DD NL.6 — `clawd git query` CLI command.
//!
//! Ask a natural language question about the git history of the current repo.
//!
//! ## Usage
//!
//! ```text
//! clawd git query "what changed last week?"
//! clawd git query "who fixed bugs in auth/"
//! ```
//!
//! Requires the daemon to be running. Uses the current directory as the repo path.

use anyhow::Result;
use serde_json::json;
use std::path::Path;

/// Run a natural language git query and print the narrative + commit list.
pub async fn query(
    question: String,
    repo_path: Option<std::path::PathBuf>,
    data_dir: &Path,
    port: u16,
) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let repo = repo_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    let result = client
        .call_once(
            "git.query",
            json!({ "question": question, "repoPath": repo }),
        )
        .await?;

    let narrative = result
        .get("narrative")
        .and_then(|v| v.as_str())
        .unwrap_or("No narrative available.");
    let commits = result
        .get("commits")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    println!("{narrative}\n");

    if commits.is_empty() {
        println!("No matching commits found.");
        return Ok(());
    }

    println!("{} commit(s):\n", commits.len());
    for c in &commits {
        let hash = c.get("hash").and_then(|v| v.as_str()).unwrap_or("?");
        let subject = c.get("subject").and_then(|v| v.as_str()).unwrap_or("");
        let author = c.get("authorName").and_then(|v| v.as_str()).unwrap_or("");
        let date = c.get("authorDate").and_then(|v| v.as_str()).unwrap_or("");
        let short: String = hash.chars().take(7).collect();
        println!("  {short}  {subject}  ({author}, {date})");
    }

    Ok(())
}
