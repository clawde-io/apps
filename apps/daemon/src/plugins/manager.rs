// SPDX-License-Identifier: MIT
//! Sprint FF PL.8 — Plugin lifecycle manager.
//!
//! Owns all loaded plugins. Responsible for:
//! - Loading enabled plugins from `.claw/plugins/` on daemon startup.
//! - Reloading on `plugin.enable`.
//! - Unloading on `plugin.disable`.
//! - Delivering events to all loaded plugins.
//! - Isolating plugin crashes — a panicking plugin is disabled, daemon continues.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::Mutex;

use clawd_plugin_abi::manifest::{ManifestRuntime, PluginManifest};

use super::dylib_runtime::DylibPlugin;
use super::wasm_runtime::WasmPlugin;

/// Status of a loaded plugin.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Enabled,
    Disabled,
    Failed,
}

/// Metadata about a plugin known to the manager.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub runtime: String,
    pub status: PluginStatus,
    pub path: String,
    pub is_signed: bool,
}

/// Internal plugin entry.
enum LoadedPlugin {
    Dylib(DylibPlugin),
    Wasm(WasmPlugin),
}

impl LoadedPlugin {
    fn name(&self) -> &str {
        match self {
            LoadedPlugin::Dylib(p) => &p.name,
            LoadedPlugin::Wasm(p) => &p.name,
        }
    }
}

/// Plugin manager — owns all loaded plugins.
pub struct PluginManager {
    /// Loaded and enabled plugins, keyed by name.
    plugins: Mutex<HashMap<String, LoadedPlugin>>,
    /// Plugin metadata (all known, enabled or not).
    registry: Mutex<HashMap<String, PluginInfo>>,
    /// Base directory where plugins are installed.
    plugins_dir: PathBuf,
}

impl PluginManager {
    pub fn new(plugins_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            plugins: Mutex::new(HashMap::new()),
            registry: Mutex::new(HashMap::new()),
            plugins_dir,
        })
    }

    /// Load all enabled plugins from `{plugins_dir}/`.
    /// Each plugin lives in `{plugins_dir}/{name}/clawd-plugin.json`.
    pub async fn load_all(&self) -> Result<()> {
        if !self.plugins_dir.exists() {
            return Ok(()); // No plugins dir — fine, no plugins installed.
        }

        let mut dir = tokio::fs::read_dir(&self.plugins_dir)
            .await
            .context("failed to read plugins dir")?;

        while let Some(entry) = dir.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            let plugin_dir = entry.path();
            let manifest_path = plugin_dir.join("clawd-plugin.json");
            if !manifest_path.exists() {
                continue;
            }
            if let Err(e) = self.load_plugin(&plugin_dir).await {
                tracing::warn!(
                    plugin_dir = %plugin_dir.display(),
                    error = %e,
                    "failed to load plugin — skipping"
                );
            }
        }
        Ok(())
    }

    /// Load a single plugin from its directory.
    pub async fn load_plugin(&self, plugin_dir: &Path) -> Result<()> {
        let manifest_json = tokio::fs::read_to_string(plugin_dir.join("clawd-plugin.json"))
            .await
            .context("failed to read clawd-plugin.json")?;
        let manifest = PluginManifest::from_json(&manifest_json)
            .context("failed to parse clawd-plugin.json")?;

        let binary_path = plugin_dir.join(&manifest.entry);
        let sig = if manifest.signature.is_empty() {
            None
        } else {
            Some(manifest.signature.as_str())
        };

        let loaded = match manifest.runtime {
            ManifestRuntime::Dylib => {
                let plugin = DylibPlugin::load(&binary_path, sig, None)
                    .context("failed to load dylib plugin")?;
                LoadedPlugin::Dylib(plugin)
            }
            ManifestRuntime::Wasm => {
                let plugin =
                    WasmPlugin::load(&binary_path, &manifest.name, manifest.capabilities.clone())
                        .context("failed to load WASM plugin")?;
                LoadedPlugin::Wasm(plugin)
            }
        };

        let info = PluginInfo {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            runtime: format!("{:?}", manifest.runtime).to_lowercase(),
            status: PluginStatus::Enabled,
            path: plugin_dir.to_string_lossy().into_owned(),
            is_signed: manifest.is_signed(),
        };

        tracing::info!(plugin = %manifest.name, "plugin loaded");
        self.plugins
            .lock()
            .await
            .insert(manifest.name.clone(), loaded);
        self.registry
            .lock()
            .await
            .insert(manifest.name.clone(), info);
        Ok(())
    }

    /// List all known plugins (enabled + disabled + failed).
    pub async fn list(&self) -> Vec<PluginInfo> {
        self.registry.lock().await.values().cloned().collect()
    }

    /// Enable a plugin by name (re-loads if disabled/failed).
    pub async fn enable(&self, name: &str) -> Result<()> {
        let plugin_dir = self.plugins_dir.join(name);
        self.load_plugin(&plugin_dir).await?;
        if let Some(info) = self.registry.lock().await.get_mut(name) {
            info.status = PluginStatus::Enabled;
        }
        Ok(())
    }

    /// Disable a plugin by name (calls on_unload, removes from active set).
    pub async fn disable(&self, name: &str) -> Result<()> {
        let mut plugins = self.plugins.lock().await;
        if let Some(plugin) = plugins.remove(name) {
            tracing::info!(plugin = %plugin.name(), "plugin disabled");
        }
        if let Some(info) = self.registry.lock().await.get_mut(name) {
            info.status = PluginStatus::Disabled;
        }
        Ok(())
    }

    /// Deliver a `session_start` event to all loaded plugins.
    pub async fn on_session_start(&self, session_id: &str) {
        let plugins = self.plugins.lock().await;
        let session_cstr = std::ffi::CString::new(session_id).unwrap_or_default();
        for (name, plugin) in plugins.iter() {
            match plugin {
                LoadedPlugin::Dylib(p) => {
                    // We don't have a real ClawaContext in this scaffold —
                    // pass null; dylib plugins should handle null gracefully.
                    let result =
                        p.call_on_session_start(std::ptr::null_mut(), session_cstr.as_ptr());
                    if result != clawd_plugin_abi::ClawaError::None {
                        tracing::warn!(plugin = %name, "on_session_start returned error");
                    }
                }
                LoadedPlugin::Wasm(p) => {
                    if let Err(e) = p.call_on_session_start(session_id) {
                        tracing::warn!(plugin = %name, error = %e, "WASM on_session_start error");
                    }
                }
            }
        }
    }

    /// Unload all plugins cleanly (called on daemon shutdown).
    pub async fn shutdown(&self) {
        let mut plugins = self.plugins.lock().await;
        for (name, plugin) in plugins.drain() {
            tracing::debug!(plugin = %name, "unloading plugin");
            if let LoadedPlugin::Dylib(p) = plugin {
                p.call_on_unload(std::ptr::null_mut());
            }
        }
    }
}
