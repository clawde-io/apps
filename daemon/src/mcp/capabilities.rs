/// MCP capability negotiation.
///
/// During the `initialize` handshake the client sends its capability set and
/// the server responds with what it supports.  `negotiate` parses the client
/// capabilities and intersects them with what `clawd` can offer.
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── ClawdCapabilities ────────────────────────────────────────────────────────

/// The set of MCP capabilities that `clawd` can advertise as a server.
///
/// Currently only `tools` is supported (v1).  `resources`, `prompts`, and
/// `sampling` are placeholders for future protocol expansions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawdCapabilities {
    /// `tools` — clawd exposes the task-management tool catalogue.
    pub tools: bool,
    /// `resources` — not yet supported.
    pub resources: bool,
    /// `prompts` — not yet supported.
    pub prompts: bool,
    /// `sampling` — not yet supported (LLM sampling delegation).
    pub sampling: bool,
}

impl Default for ClawdCapabilities {
    fn default() -> Self {
        Self {
            tools: true,
            resources: false,
            prompts: false,
            sampling: false,
        }
    }
}

impl ClawdCapabilities {
    /// Convert to the JSON object expected in an MCP `initialize` response.
    ///
    /// Only present the capabilities that are both supported by `clawd` and
    /// requested by the client.
    pub fn to_mcp_value(&self) -> Value {
        let mut cap = serde_json::Map::new();

        if self.tools {
            cap.insert(
                "tools".into(),
                serde_json::json!({ "listChanged": false }),
            );
        }
        if self.resources {
            cap.insert("resources".into(), serde_json::json!({}));
        }
        if self.prompts {
            cap.insert("prompts".into(), serde_json::json!({}));
        }
        if self.sampling {
            cap.insert("sampling".into(), serde_json::json!({}));
        }

        Value::Object(cap)
    }
}

// ─── Negotiation ──────────────────────────────────────────────────────────────

/// Parse the client's `capabilities` object and return the intersection with
/// what `clawd` supports.
///
/// The MCP spec says the server MUST NOT offer capabilities the client did not
/// request.  We honour that here: if the client's caps don't include `tools`,
/// we disable ours too (even though we have them).
///
/// `client_caps` — the raw `capabilities` field from the `initialize` params.
pub fn negotiate(client_caps: Value) -> ClawdCapabilities {
    let our_defaults = ClawdCapabilities::default();

    // Client indicates support for a capability by its presence (any value).
    let client_wants_tools = client_caps.get("tools").is_some();
    let client_wants_resources = client_caps.get("resources").is_some();
    let client_wants_prompts = client_caps.get("prompts").is_some();
    let client_wants_sampling = client_caps.get("sampling").is_some();

    ClawdCapabilities {
        tools: our_defaults.tools && client_wants_tools,
        resources: our_defaults.resources && client_wants_resources,
        prompts: our_defaults.prompts && client_wants_prompts,
        sampling: our_defaults.sampling && client_wants_sampling,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn negotiate_tools_only() {
        let client_caps = json!({ "tools": {} });
        let caps = negotiate(client_caps);
        assert!(caps.tools);
        assert!(!caps.resources);
        assert!(!caps.prompts);
        assert!(!caps.sampling);
    }

    #[test]
    fn negotiate_empty_client() {
        // Client that doesn't advertise any capabilities should get nothing.
        let caps = negotiate(json!({}));
        assert!(!caps.tools);
        assert!(!caps.resources);
    }

    #[test]
    fn default_has_tools() {
        let defaults = ClawdCapabilities::default();
        assert!(defaults.tools);
        assert!(!defaults.resources);
    }

    #[test]
    fn to_mcp_value_tools_only() {
        let caps = ClawdCapabilities {
            tools: true,
            resources: false,
            prompts: false,
            sampling: false,
        };
        let v = caps.to_mcp_value();
        assert!(v.get("tools").is_some());
        assert!(v.get("resources").is_none());
    }
}
