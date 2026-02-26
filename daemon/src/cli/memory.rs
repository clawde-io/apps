// cli/memory.rs — `clawd memory list/add/remove/show` CLI commands.
//
// Sprint OO ME.3

use anyhow::Result;
use serde_json::json;

use super::client::DaemonClient;

/// `clawd memory list [--scope global|proj:...]`
pub async fn cmd_list(scope: Option<String>, repo_path: Option<String>) -> Result<()> {
    let client = DaemonClient::connect().await?;
    let params = if let Some(path) = repo_path {
        json!({ "repo_path": path, "include_global": true })
    } else {
        json!({ "scope": scope.unwrap_or_else(|| "global".to_string()), "include_global": false })
    };

    let result = client.call("memory.list", params).await?;
    let entries = result["entries"].as_array().cloned().unwrap_or_default();

    if entries.is_empty() {
        println!("No memory entries found.");
        return Ok(());
    }

    println!("{:<40} {:<10} {:<8} {}", "Key", "Scope", "Weight", "Value");
    println!("{}", "-".repeat(80));
    for entry in &entries {
        let key = entry["key"].as_str().unwrap_or("");
        let scope = entry["scope"].as_str().unwrap_or("");
        let weight = entry["weight"].as_i64().unwrap_or(5);
        let value = entry["value"].as_str().unwrap_or("");
        let value_preview = if value.len() > 40 {
            format!("{}…", &value[..40])
        } else {
            value.to_string()
        };
        println!("{:<40} {:<10} {:<8} {}", key, scope, weight, value_preview);
    }
    println!("\n{} entries", entries.len());
    Ok(())
}

/// `clawd memory add <key> <value> [--scope global] [--weight 5]`
pub async fn cmd_add(key: String, value: String, scope: String, weight: i64) -> Result<()> {
    let client = DaemonClient::connect().await?;
    let result = client
        .call(
            "memory.add",
            json!({ "scope": scope, "key": key, "value": value, "weight": weight }),
        )
        .await?;
    let entry = &result["entry"];
    println!(
        "✓ Memory entry added: {} = {:?} (weight: {}, scope: {})",
        entry["key"].as_str().unwrap_or(""),
        entry["value"].as_str().unwrap_or(""),
        entry["weight"].as_i64().unwrap_or(5),
        entry["scope"].as_str().unwrap_or(""),
    );
    Ok(())
}

/// `clawd memory remove <id>`
pub async fn cmd_remove(id: String) -> Result<()> {
    let client = DaemonClient::connect().await?;
    let result = client
        .call("memory.remove", json!({ "id": id }))
        .await?;
    if result["removed"].as_bool().unwrap_or(false) {
        println!("✓ Removed memory entry: {}", id);
    } else {
        println!("No entry found with ID: {}", id);
    }
    Ok(())
}

/// `clawd memory show <key> [--scope global]`
pub async fn cmd_show(key: String, scope: String) -> Result<()> {
    let client = DaemonClient::connect().await?;
    let result = client
        .call("memory.list", json!({ "scope": scope }))
        .await?;
    let entries = result["entries"].as_array().cloned().unwrap_or_default();
    let entry = entries.iter().find(|e| e["key"].as_str() == Some(&key));
    match entry {
        Some(e) => {
            println!("Key:    {}", e["key"].as_str().unwrap_or(""));
            println!("Value:  {}", e["value"].as_str().unwrap_or(""));
            println!("Scope:  {}", e["scope"].as_str().unwrap_or(""));
            println!("Weight: {}", e["weight"].as_i64().unwrap_or(5));
            println!("Source: {}", e["source"].as_str().unwrap_or(""));
        }
        None => println!("No entry found for key: {} in scope: {}", key, scope),
    }
    Ok(())
}
