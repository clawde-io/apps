//! Sprint DD WR.4 — `clawd recipe` CLI commands.
//!
//! Subcommands:
//!   - `clawd recipe list`        — list all workflow recipes
//!   - `clawd recipe run <id>`    — run a recipe in the current repo
//!   - `clawd recipe import <path>` — import a YAML workflow from disk
//!
//! Requires the daemon to be running.

use anyhow::Result;
use serde_json::json;
use std::path::Path;

/// List all available workflow recipes.
pub async fn list(data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client.call_once("workflow.list", json!({})).await?;

    let recipes = result
        .get("recipes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if recipes.is_empty() {
        println!("No workflow recipes found.");
        return Ok(());
    }

    println!("{} recipe(s):\n", recipes.len());
    for r in &recipes {
        let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let builtin = r.get("isBuiltin").and_then(|v| v.as_bool()).unwrap_or(false);
        let runs = r.get("runCount").and_then(|v| v.as_u64()).unwrap_or(0);
        let badge = if builtin { " [built-in]" } else { "" };

        println!("  {id}{badge}  {name}  ({runs} runs)");
        if !desc.is_empty() {
            println!("    {desc}");
        }
    }

    Ok(())
}

/// Run a workflow recipe by ID in the current (or specified) repo.
pub async fn run(
    recipe_id: String,
    repo_path: Option<std::path::PathBuf>,
    data_dir: &Path,
    port: u16,
) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let repo = repo_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    println!("Starting workflow '{recipe_id}' in {repo} ...");

    let result = client
        .call_once(
            "workflow.run",
            json!({ "recipeId": recipe_id, "repoPath": repo }),
        )
        .await?;

    let run_id = result.get("runId").and_then(|v| v.as_str()).unwrap_or("?");
    let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("?");

    println!("Workflow started — run ID: {run_id}  status: {status}");
    println!("Steps will execute in the background. Watch the desktop app for progress.");

    Ok(())
}

/// Import a workflow recipe from a YAML file on disk.
pub async fn import(yaml_path: std::path::PathBuf, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let yaml = std::fs::read_to_string(&yaml_path)
        .map_err(|e| anyhow::anyhow!("could not read {}: {e}", yaml_path.display()))?;

    // Derive a name from the filename
    let stem = yaml_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported")
        .to_string();

    let result = client
        .call_once(
            "workflow.create",
            json!({ "name": stem, "description": "", "yaml": yaml }),
        )
        .await?;

    let id = result.get("id").and_then(|v| v.as_str()).unwrap_or("?");
    println!("Imported workflow '{stem}' — id: {id}");

    Ok(())
}
