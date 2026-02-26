//! Sprint DD TS.4 — `clawd sovereignty report` CLI command.
//!
//! Prints a 7-day summary of other AI tools that have been detected writing
//! files to the current project.
//!
//! ## Usage
//!
//! ```text
//! clawd sovereignty report
//! ```
//!
//! Requires the daemon to be running.

use anyhow::Result;
use serde_json::json;
use std::path::Path;

/// Print the 7-day sovereignty report to stdout.
pub async fn report(data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client.call_once("sovereignty.report", json!({})).await?;

    let tools = result
        .get("tools")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total_events = result
        .get("totalEvents")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if tools.is_empty() {
        println!("No other AI tools detected in the last 7 days.");
        println!("This codebase is ClawDE-exclusive.");
        return Ok(());
    }

    println!(
        "{} other AI tool(s) detected — {} event(s) in last 7 days:\n",
        tools.len(),
        total_events
    );

    for tool in &tools {
        let tool_id = tool.get("toolId").and_then(|v| v.as_str()).unwrap_or("?");
        let event_count = tool.get("eventCount").and_then(|v| v.as_u64()).unwrap_or(0);
        let last_seen = tool
            .get("lastSeen")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let files: Vec<&str> = tool
            .get("filesTouched")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        println!("  {tool_id}");
        println!("    events: {event_count}  last seen: {last_seen}");
        if !files.is_empty() {
            let preview: Vec<&str> = files.iter().take(3).copied().collect();
            let extra = files.len().saturating_sub(3);
            if extra > 0 {
                println!("    files:  {} +{extra} more", preview.join(", "));
            } else {
                println!("    files:  {}", preview.join(", "));
            }
        }
        println!();
    }

    Ok(())
}
