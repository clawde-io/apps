// SPDX-License-Identifier: MIT
//! Plugin manifest format — `clawd-plugin.json`.
//!
//! Every plugin pack must include a `clawd-plugin.json` manifest at the
//! pack root. The daemon reads this before loading the plugin binary.

use serde::{Deserialize, Serialize};

/// Runtime type string as it appears in `clawd-plugin.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestRuntime {
    /// Native dynamic library.
    Dylib,
    /// WebAssembly module.
    Wasm,
}

/// Capability string as it appears in `clawd-plugin.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestCapability {
    /// Read files in the project directory.
    #[serde(rename = "fs.read")]
    FsRead,
    /// Write files in the project directory.
    #[serde(rename = "fs.write")]
    FsWrite,
    /// Send events to connected clients.
    #[serde(rename = "network.relay")]
    NetworkRelay,
    /// Call daemon RPC methods.
    #[serde(rename = "daemon.rpc")]
    DaemonRpc,
}

/// Contents of a `clawd-plugin.json` manifest file.
///
/// # Example
///
/// ```json
/// {
///   "name": "hello-clawd",
///   "version": "1.0.0",
///   "runtime": "dylib",
///   "entry": "libhello_clawd.dylib",
///   "capabilities": ["fs.read"],
///   "signature": "base64-ed25519-sig-here"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin display name (e.g. `"hello-clawd"`).
    pub name: String,

    /// Semver version string (e.g. `"1.0.0"`).
    pub version: String,

    /// Plugin description shown in the UI.
    #[serde(default)]
    pub description: String,

    /// Author or publisher identifier.
    #[serde(default)]
    pub author: String,

    /// Runtime type: `"dylib"` or `"wasm"`.
    pub runtime: ManifestRuntime,

    /// Relative path to the plugin binary inside the pack.
    pub entry: String,

    /// Capabilities the plugin requires. The daemon will prompt the user
    /// to approve these on first install.
    #[serde(default)]
    pub capabilities: Vec<ManifestCapability>,

    /// Base64-encoded Ed25519 signature over the plugin binary.
    /// Required for plugins distributed through the official registry.
    /// Self-signed plugins may omit this field (user is prompted).
    #[serde(default)]
    pub signature: String,
}

impl PluginManifest {
    /// Parse a manifest from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize the manifest to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Returns true if the manifest has a non-empty signature field.
    pub fn is_signed(&self) -> bool {
        !self.signature.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let json = r#"{
            "name": "hello-clawd",
            "version": "1.0.0",
            "runtime": "dylib",
            "entry": "libhello_clawd.dylib"
        }"#;
        let m = PluginManifest::from_json(json).unwrap();
        assert_eq!(m.name, "hello-clawd");
        assert_eq!(m.runtime, ManifestRuntime::Dylib);
        assert!(m.capabilities.is_empty());
        assert!(!m.is_signed());
    }

    #[test]
    fn parse_full_manifest() {
        let json = r#"{
            "name": "auto-test",
            "version": "0.2.1",
            "description": "Runs tests on every task_done event",
            "author": "clawde-io",
            "runtime": "wasm",
            "entry": "auto_test.wasm",
            "capabilities": ["fs.read", "daemon.rpc"],
            "signature": "aabbcc112233"
        }"#;
        let m = PluginManifest::from_json(json).unwrap();
        assert_eq!(m.runtime, ManifestRuntime::Wasm);
        assert_eq!(m.capabilities.len(), 2);
        assert!(m.is_signed());
    }

    #[test]
    fn roundtrip_serialization() {
        let m = PluginManifest {
            name: "test-plugin".into(),
            version: "1.0.0".into(),
            description: "A test plugin".into(),
            author: "test".into(),
            runtime: ManifestRuntime::Dylib,
            entry: "libtest.dylib".into(),
            capabilities: vec![ManifestCapability::FsRead],
            signature: String::new(),
        };
        let json = m.to_json().unwrap();
        let m2 = PluginManifest::from_json(&json).unwrap();
        assert_eq!(m.name, m2.name);
        assert_eq!(m.capabilities.len(), m2.capabilities.len());
    }
}
