// cli/providers.rs — `clawd providers list --capabilities` (Sprint ZZ MP.T04)

use anyhow::Result;
use serde_json::json;
use std::path::Path;

/// MP.T04 — `clawd providers list --capabilities`
pub async fn list_capabilities(data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client.call_once("providers.listCapabilities", json!({})).await?;

    let providers = result["providers"].as_array().cloned().unwrap_or_default();

    if providers.is_empty() {
        println!("No providers configured.");
        return Ok(());
    }

    // Header
    println!(
        "{:<14} {:<12} {:<10} {:<10} {:<10} {:<10} {:<10}",
        "Provider", "Installed", "Version", "Session", "Code Gen", "Review", "LSP"
    );
    println!("{}", "─".repeat(80));

    for provider in &providers {
        let name = provider["name"].as_str().unwrap_or("?");
        let installed = provider["installed"].as_bool().unwrap_or(false);
        let version = provider["version"].as_str().unwrap_or("-");
        let caps = &provider["capabilities"];

        let full_session = cap_icon(caps, "full_session");
        let code_gen = cap_icon(caps, "code_generation");
        let review = cap_icon(caps, "code_review");
        let lsp = cap_icon(caps, "lsp_integration");
        let installed_str = if installed { "✓" } else { "✗" };

        println!(
            "{:<14} {:<12} {:<10} {:<10} {:<10} {:<10} {:<10}",
            name, installed_str, version, full_session, code_gen, review, lsp
        );
    }

    println!("\n✓ = supported  ✗ = not supported  - = unknown");
    Ok(())
}

fn cap_icon(caps: &serde_json::Value, key: &str) -> &'static str {
    match caps.get(key) {
        Some(v) if v.as_bool() == Some(true) => "✓",
        Some(v) if v.as_bool() == Some(false) => "✗",
        _ => "-",
    }
}
