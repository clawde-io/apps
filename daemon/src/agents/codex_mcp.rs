//! Connects to Codex in MCP server mode.
//!
//! `codex mcp-server` spawns a stdio MCP server.  This module wraps the
//! existing `McpClient` to provide a typed, Codex-specific façade.

use anyhow::{Context, Result};
use std::collections::HashMap;

use crate::mcp::client::{McpClient, McpServerConfig, McpTrustLevel};

/// Timeout for the MCP server initialization handshake.
const MCP_INIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// ─── CodexMcpServer ───────────────────────────────────────────────────────────

/// A live connection to a `codex mcp-server` subprocess.
pub struct CodexMcpServer {
    client: McpClient,
    pub server_name: String,
}

impl CodexMcpServer {
    /// Spawn `codex mcp-server` as a child process and perform the MCP
    /// `initialize` handshake.
    ///
    /// Fails with an error if the handshake does not complete within 10 seconds.
    pub async fn spawn() -> Result<Self> {
        let config = McpServerConfig {
            name: "codex".to_string(),
            command: "codex".to_string(),
            args: vec!["mcp-server".to_string()],
            env: HashMap::new(),
            // Codex is a first-party provider; trust its responses.
            trust: McpTrustLevel::Trusted,
        };
        let client = tokio::time::timeout(MCP_INIT_TIMEOUT, McpClient::spawn(config))
            .await
            .context("MCP server initialization timed out after 10 seconds")?
            .context("MCP server spawn failed")?;
        Ok(Self {
            client,
            server_name: "codex".to_string(),
        })
    }

    /// List all tools exposed by the Codex MCP server.
    pub async fn list_tools(&self) -> Result<serde_json::Value> {
        let tools = self.client.list_tools().await?;
        Ok(serde_json::to_value(tools)?)
    }

    /// Call a named tool on the Codex MCP server.
    pub async fn call_tool(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.client.call_tool(name, args).await
    }
}
