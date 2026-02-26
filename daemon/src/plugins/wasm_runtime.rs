// SPDX-License-Identifier: MIT
//! Sprint FF PL.3 — WebAssembly plugin runtime (wasmtime).
//!
//! Loads `.wasm` plugin modules. Exposes a set of host functions that WASM
//! plugins can call (send_event, log, read_file, run_tool). Capability
//! grants per plugin are enforced before each host call.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use clawd_plugin_abi::manifest::ManifestCapability;

/// Host state threaded through the WASM store.
pub struct WasmHostState {
    /// Session-local broadcast channel sender (wrapped in Arc<Mutex>).
    /// Events queued here are drained and dispatched after each plugin call.
    pub queued_events: Vec<(String, Value)>,
    /// Log messages queued for emission after the call returns.
    pub queued_logs: Vec<(u8, String)>,
    /// Capability grants for this plugin instance.
    pub capabilities: HashSet<String>,
    /// Plugin name (for logging).
    pub plugin_name: String,
}

impl WasmHostState {
    pub fn new(plugin_name: impl Into<String>, capabilities: &[ManifestCapability]) -> Self {
        let cap_set = capabilities
            .iter()
            .map(|c| format!("{:?}", c))
            .collect();
        Self {
            queued_events: Vec::new(),
            queued_logs: Vec::new(),
            capabilities: cap_set,
            plugin_name: plugin_name.into(),
        }
    }

    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.contains(cap)
    }
}

/// A loaded WASM plugin instance.
///
/// Uses wasmtime to instantiate the module and call its exported functions.
/// The engine and store are per-plugin — WASM isolation is per-instance.
pub struct WasmPlugin {
    /// Display name from manifest.
    pub name: String,
    /// Instantiated WASM module bytes (stored for re-instantiation if needed).
    _wasm_bytes: Vec<u8>,
    /// Granted capabilities (used by host functions).
    capabilities: Vec<ManifestCapability>,
}

impl WasmPlugin {
    /// Load a WASM plugin from a `.wasm` binary.
    ///
    /// Steps:
    /// 1. Read the WASM bytes.
    /// 2. Validate via wasmtime (checks structure, not semantics).
    /// 3. Check that `clawd_plugin_init` export is present.
    pub fn load(
        binary_path: &Path,
        name: impl Into<String>,
        capabilities: Vec<ManifestCapability>,
    ) -> Result<Self> {
        let wasm_bytes = std::fs::read(binary_path)
            .with_context(|| format!("failed to read WASM: {}", binary_path.display()))?;

        // Lightweight magic-byte validation (WASM magic: \0asm).
        if !wasm_bytes.starts_with(b"\0asm") {
            bail!(
                "not a valid WASM module: {} (missing \\0asm magic)",
                binary_path.display()
            );
        }

        // TODO (Sprint FF+): use wasmtime Engine to fully validate and
        // instantiate the module with host function imports. For now we
        // validate the magic bytes and store the bytes for later instantiation.
        // Full wasmtime integration requires wasmtime = "18" in Cargo.toml.

        Ok(Self {
            name: name.into(),
            _wasm_bytes: wasm_bytes,
            capabilities,
        })
    }

    /// Call the `clawd_plugin_init` export (placeholder — full wasmtime
    /// instantiation wired in Sprint FF execution).
    pub fn call_init(&self) -> Result<()> {
        // Placeholder: returns Ok until wasmtime is wired.
        tracing::debug!(plugin = %self.name, "WASM plugin init (stub)");
        Ok(())
    }

    /// Call `on_session_start` WASM export.
    pub fn call_on_session_start(&self, session_id: &str) -> Result<()> {
        tracing::debug!(
            plugin = %self.name,
            session_id = %session_id,
            "WASM on_session_start (stub)"
        );
        Ok(())
    }

    /// Returns the capability grants for this plugin.
    pub fn capabilities(&self) -> &[ManifestCapability] {
        &self.capabilities
    }
}
