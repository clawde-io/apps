//! Sprint DD SR.3 — `clawd session export/import` CLI commands.
//!
//! Subcommands:
//!   - `clawd session export <session-id> [--out <file.json.gz>]`
//!   - `clawd session import <file.json.gz>`
//!
//! The export command writes a base64-encoded gzip bundle to disk.
//! The import command reads it back and creates a replay session.

use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Export a session to a portable bundle file.
pub async fn export(
    session_id: String,
    out_path: Option<PathBuf>,
    data_dir: &Path,
    port: u16,
) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    println!("Exporting session {session_id} ...");

    let result = client
        .call_once("session.export", json!({ "sessionId": session_id }))
        .await?;

    let bundle = result
        .get("bundle")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("daemon returned no bundle data"))?;
    let message_count = result
        .get("messageCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // Write to file
    let target = out_path.unwrap_or_else(|| {
        let short: String = session_id.chars().take(8).collect();
        PathBuf::from(format!("clawd-session-{short}.clawd"))
    });

    std::fs::write(&target, bundle)
        .map_err(|e| anyhow::anyhow!("could not write {}: {e}", target.display()))?;

    println!(
        "Exported {message_count} message(s) to {}",
        target.display()
    );

    Ok(())
}

/// Import a session bundle and create a replay session.
pub async fn import(bundle_path: PathBuf, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let bundle = std::fs::read_to_string(&bundle_path)
        .map_err(|e| anyhow::anyhow!("could not read {}: {e}", bundle_path.display()))?;

    println!("Importing session from {} ...", bundle_path.display());

    let result = client
        .call_once("session.import", json!({ "bundle": bundle }))
        .await?;

    let session_id = result
        .get("sessionId")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let imported = result
        .get("importedMessages")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    println!("Imported {imported} message(s) — replay session: {session_id}");
    println!("Open the desktop app to replay this session.");

    Ok(())
}
