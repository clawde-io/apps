//! Generate Claude Code `settings.json` from `.claw/` policies.
//!
//! `clawd init-claw` writes a managed `settings.json` into the project's
//! `.claude/` directory so that Claude Code picks up the correct permission
//! allow/deny lists and the `clawd` MCP server registration.

use std::path::Path;

use serde_json::json;

// ─── Config generation ────────────────────────────────────────────────────────

/// Generate the JSON value for Claude Code's `settings.json`.
///
/// The settings file:
/// - Allows safe read-only Bash commands and file-system reads.
/// - Denies destructive Bash operations (`rm`, `curl`, `wget`).
/// - Registers the `clawd` MCP server so Claude Code routes tool calls
///   through the daemon's task governance layer.
pub fn generate_claude_settings(claw_dir: &Path) -> serde_json::Value {
    // claw_dir is accepted as a parameter for future per-policy expansion;
    // currently the deny list is static but will be derived from
    // `.claw/policies/tool-risk.json` in a later phase.
    let _ = claw_dir;

    json!({
        "permissions": {
            "allow": [
                "Bash(read:*)",
                "Read(*)",
                "Glob(*)",
                "Grep(*)"
            ],
            "deny": [
                "Bash(rm:*)",
                "Bash(curl:*)",
                "Bash(wget:*)"
            ]
        },
        "mcpServers": {
            "clawd": {
                "command": "clawd",
                "args": ["mcp-serve"],
                "env": {}
            }
        }
    })
}

/// Write `.claude/settings.json` into the given project directory.
///
/// Creates the `.claude/` directory if it does not exist.
pub async fn write_claude_settings(project_dir: &Path, claw_dir: &Path) -> anyhow::Result<()> {
    let claude_dir = project_dir.join(".claude");
    tokio::fs::create_dir_all(&claude_dir).await?;
    let settings = generate_claude_settings(claw_dir);
    let content = serde_json::to_string_pretty(&settings)?;
    tokio::fs::write(claude_dir.join("settings.json"), content).await?;
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_has_permissions_and_mcp() {
        let claw = std::path::Path::new("/tmp/test/.claw");
        let v = generate_claude_settings(claw);
        assert!(v.get("permissions").is_some(), "missing permissions");
        assert!(v.get("mcpServers").is_some(), "missing mcpServers");
    }

    #[test]
    fn settings_registers_clawd_mcp_server() {
        let claw = std::path::Path::new("/tmp/test/.claw");
        let v = generate_claude_settings(claw);
        let servers = v.get("mcpServers").expect("mcpServers present");
        assert!(
            servers.get("clawd").is_some(),
            "clawd server not registered"
        );
    }

    #[test]
    fn settings_denies_rm() {
        let claw = std::path::Path::new("/tmp/test/.claw");
        let v = generate_claude_settings(claw);
        let deny = v["permissions"]["deny"].as_array().expect("deny is array");
        let has_rm = deny.iter().any(|d| d.as_str() == Some("Bash(rm:*)"));
        assert!(has_rm, "Bash(rm:*) not in deny list");
    }
}
