//! MCP server trust registry.
//!
//! Each MCP server that connects to `clawd` must be present in
//! `.claw/policies/mcp-trust.json` with trust level `Trusted` before its
//! tools can be dispatched. Servers not in the registry default to `Untrusted`.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::warn;

// ─── Trust types ──────────────────────────────────────────────────────────────

/// How much a given MCP server is trusted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Trusted,
    Untrusted,
}

/// A single entry in the trust registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTrustEntry {
    /// Name of the MCP server (matches the `name` field in the MCP `initialize` handshake).
    pub server_name: String,
    /// Trust level assigned to this server.
    pub trust: TrustLevel,
    /// Optional SHA-256 hash of the server binary/command that was in use when
    /// the server was first registered.  A hash mismatch triggers a supply-chain
    /// violation.
    pub command_hash: Option<String>,
    /// Allow-list of tool names this server may invoke.  An empty list means
    /// all tools on that server are permitted (subject to other policy checks).
    pub allowed_tools: Vec<String>,
}

// ─── Trust database ───────────────────────────────────────────────────────────

/// In-memory trust registry loaded from `mcp-trust.json`.
#[derive(Debug, Default)]
pub struct TrustDatabase {
    entries: HashMap<String, McpTrustEntry>,
}

/// JSON shape expected in `mcp-trust.json`.
#[derive(Debug, Deserialize)]
struct TrustConfigFile {
    #[serde(default)]
    servers: Vec<McpTrustEntry>,
}

impl TrustDatabase {
    /// Empty registry — all servers are `Untrusted`.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Load from `mcp-trust.json`.  Missing or malformed files return an empty
    /// (all-untrusted) database with a warning.
    pub fn load(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!(path = %path.display(), err = %e, "mcp-trust.json not found — all servers untrusted");
                return Self::default();
            }
        };

        let config: TrustConfigFile = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                warn!(err = %e, "mcp-trust.json parse error — all servers untrusted");
                return Self::default();
            }
        };

        let mut entries = HashMap::new();
        for entry in config.servers {
            entries.insert(entry.server_name.clone(), entry);
        }

        Self { entries }
    }

    /// Return the trust level for a server.  Defaults to `Untrusted`.
    pub fn get_trust(&self, server_name: &str) -> TrustLevel {
        self.entries
            .get(server_name)
            .map(|e| e.trust.clone())
            .unwrap_or(TrustLevel::Untrusted)
    }

    /// Check whether a specific tool is on the allow-list for `server_name`.
    ///
    /// Returns `true` when:
    /// - The server is `Trusted`, AND
    /// - Either the allowed_tools list is empty (all tools allowed) or the tool
    ///   is explicitly listed.
    pub fn is_tool_allowed(&self, server_name: &str, tool: &str) -> bool {
        let Some(entry) = self.entries.get(server_name) else {
            return false;
        };

        if entry.trust != TrustLevel::Trusted {
            return false;
        }

        if entry.allowed_tools.is_empty() {
            return true;
        }

        entry.allowed_tools.iter().any(|t| t == tool)
    }

    /// Insert or replace an entry (used by supply_chain after verification).
    pub fn upsert(&mut self, entry: McpTrustEntry) {
        self.entries.insert(entry.server_name.clone(), entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_with_trusted_server() -> TrustDatabase {
        let mut db = TrustDatabase::default();
        db.upsert(McpTrustEntry {
            server_name: "my-mcp".to_string(),
            trust: TrustLevel::Trusted,
            command_hash: None,
            allowed_tools: vec![],
        });
        db
    }

    #[test]
    fn unknown_server_is_untrusted() {
        let db = TrustDatabase::default();
        assert_eq!(db.get_trust("unknown"), TrustLevel::Untrusted);
    }

    #[test]
    fn known_trusted_server() {
        let db = db_with_trusted_server();
        assert_eq!(db.get_trust("my-mcp"), TrustLevel::Trusted);
    }

    #[test]
    fn trusted_server_all_tools_allowed() {
        let db = db_with_trusted_server();
        assert!(db.is_tool_allowed("my-mcp", "any_tool"));
    }

    #[test]
    fn untrusted_server_tool_denied() {
        let db = TrustDatabase::default();
        assert!(!db.is_tool_allowed("unknown-server", "apply_patch"));
    }

    #[test]
    fn allowed_tools_list_respected() {
        let mut db = TrustDatabase::default();
        db.upsert(McpTrustEntry {
            server_name: "limited-mcp".to_string(),
            trust: TrustLevel::Trusted,
            command_hash: None,
            allowed_tools: vec!["read_file".to_string()],
        });

        assert!(db.is_tool_allowed("limited-mcp", "read_file"));
        assert!(!db.is_tool_allowed("limited-mcp", "apply_patch"));
    }
}
