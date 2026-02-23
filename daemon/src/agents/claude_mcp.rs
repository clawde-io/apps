//! Configure Claude Code to use `clawd` as its MCP server.
//!
//! Writes `managed-mcp.json` into the project's `.claude/` directory.
//! Claude Code reads this file to register MCP servers under managed
//! (daemon-controlled) configuration, as distinct from user-editable
//! `settings.json`.

use std::path::Path;

use serde_json::json;

// ─── Config generation ────────────────────────────────────────────────────────

/// Generate the JSON value for Claude Code's `managed-mcp.json`.
///
/// Registers the `clawd` daemon as a trusted MCP server so that all tool
/// calls from Claude Code are routed through the daemon's governance layer.
pub fn generate_managed_mcp_config() -> serde_json::Value {
    json!({
        "mcpServers": {
            "clawd": {
                "command": "clawd",
                "args": ["mcp-serve"],
                "description": "ClawDE daemon — task governance and tool broker",
                "trusted": true
            }
        }
    })
}

/// Write `.claude/managed-mcp.json` into the given project directory.
///
/// Creates the `.claude/` directory if it does not exist.
pub async fn write_managed_mcp(project_dir: &Path) -> anyhow::Result<()> {
    let claude_dir = project_dir.join(".claude");
    tokio::fs::create_dir_all(&claude_dir).await?;
    let config = generate_managed_mcp_config();
    let content = serde_json::to_string_pretty(&config)?;
    tokio::fs::write(claude_dir.join("managed-mcp.json"), content).await?;
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_registers_clawd() {
        let v = generate_managed_mcp_config();
        let clawd = &v["mcpServers"]["clawd"];
        assert_eq!(clawd["command"].as_str(), Some("clawd"));
        assert_eq!(clawd["trusted"].as_bool(), Some(true));
    }

    #[test]
    fn config_args_contain_mcp_serve() {
        let v = generate_managed_mcp_config();
        let args = v["mcpServers"]["clawd"]["args"]
            .as_array()
            .expect("args is array");
        assert!(
            args.iter().any(|a| a.as_str() == Some("mcp-serve")),
            "mcp-serve not in args"
        );
    }

    #[tokio::test]
    async fn write_creates_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_managed_mcp(dir.path())
            .await
            .expect("write_managed_mcp");
        let path = dir.path().join(".claude").join("managed-mcp.json");
        assert!(path.exists(), "managed-mcp.json not created");
        let content = std::fs::read_to_string(&path).expect("read file");
        let v: serde_json::Value = serde_json::from_str(&content).expect("valid json");
        assert!(v["mcpServers"]["clawd"]["trusted"]
            .as_bool()
            .unwrap_or(false));
    }
}
