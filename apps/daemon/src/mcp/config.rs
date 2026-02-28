/// MCP server configuration loader.
///
/// Reads `.claw/mcp-servers.json` from the project's data directory.
/// If the file does not exist, returns an empty server list (no error).
///
/// Format of `mcp-servers.json`:
/// ```json
/// {
///   "servers": [
///     {
///       "name": "my-mcp-server",
///       "command": "npx",
///       "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"],
///       "env": { "MY_KEY": "value" },
///       "trust": "trusted"
///     }
///   ]
/// }
/// ```
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, warn};

use super::client::{McpServerConfig, McpTrustLevel};

// ─── Raw JSON types (for deserialization) ─────────────────────────────────────

/// The trust level as it appears in JSON.  Serde maps "trusted" → `Trusted`,
/// "untrusted" → `Untrusted`; anything else defaults to `Untrusted`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum JsonTrustLevel {
    Trusted,
    Untrusted,
}

impl From<JsonTrustLevel> for McpTrustLevel {
    fn from(j: JsonTrustLevel) -> Self {
        match j {
            JsonTrustLevel::Trusted => McpTrustLevel::Trusted,
            JsonTrustLevel::Untrusted => McpTrustLevel::Untrusted,
        }
    }
}

/// One server entry as it appears in `mcp-servers.json`.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct JsonServerEntry {
    /// Display name.
    name: String,
    /// Executable command.
    command: String,
    /// Arguments to the command.
    #[serde(default)]
    args: Vec<String>,
    /// Environment variables to inject.
    #[serde(default)]
    env: HashMap<String, String>,
    /// Trust level.  Defaults to `untrusted` if omitted.
    #[serde(default = "default_trust")]
    trust: JsonTrustLevel,
}

fn default_trust() -> JsonTrustLevel {
    JsonTrustLevel::Untrusted
}

/// The top-level structure of `mcp-servers.json`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct JsonMcpServersFile {
    #[serde(default)]
    servers: Vec<JsonServerEntry>,
}

// ─── McpServersConfig ─────────────────────────────────────────────────────────

/// Parsed MCP server configuration.
#[derive(Debug, Clone, Default)]
pub struct McpServersConfig {
    /// All configured upstream MCP servers.
    pub servers: Vec<McpServerConfig>,
}

impl McpServersConfig {
    /// Load the configuration from `{data_dir}/.claw/mcp-servers.json`.
    ///
    /// Returns an empty config (no servers) if the file does not exist.
    /// Returns `Err` only for I/O errors other than "not found" or for JSON
    /// parse failures.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let config_path = data_dir.join(".claw").join("mcp-servers.json");

        if !config_path.exists() {
            debug!(
                path = %config_path.display(),
                "mcp-servers.json not found — no upstream MCP servers configured"
            );
            return Ok(Self::default());
        }

        let raw = std::fs::read_to_string(&config_path).map_err(|e| {
            anyhow::anyhow!(
                "failed to read mcp-servers.json at '{}': {}",
                config_path.display(),
                e
            )
        })?;

        let parsed: JsonMcpServersFile = serde_json::from_str(&raw).map_err(|e| {
            anyhow::anyhow!(
                "invalid mcp-servers.json at '{}': {}",
                config_path.display(),
                e
            )
        })?;

        let servers: Vec<McpServerConfig> = parsed
            .servers
            .into_iter()
            .map(|entry| {
                if entry.trust == JsonTrustLevel::Untrusted {
                    warn!(
                        server = %entry.name,
                        "MCP server configured with Untrusted level — responses will be sanitized"
                    );
                }
                McpServerConfig {
                    name: entry.name,
                    command: entry.command,
                    args: entry.args,
                    env: entry.env,
                    trust: entry.trust.into(),
                }
            })
            .collect();

        debug!(count = servers.len(), "loaded MCP server configs");

        Ok(Self { servers })
    }
}
