// SPDX-License-Identifier: MIT
//! Sprint FF PL.6 — Plugin management RPC handlers.
//!
//! Methods:
//! - `plugin.list`    — list all installed plugins with status
//! - `plugin.enable`  — enable a disabled plugin (re-loads it)
//! - `plugin.disable` — disable a running plugin (calls on_unload)
//! - `plugin.info`    — get detail for a single plugin

use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};

use crate::plugins::manager::PluginManager;

/// `plugin.list` — list all known plugins.
pub async fn list(manager: Arc<PluginManager>, _params: Value) -> Result<Value> {
    let plugins = manager.list().await;
    Ok(json!({ "plugins": plugins }))
}

/// `plugin.enable` — enable a plugin by name.
pub async fn enable(manager: Arc<PluginManager>, params: Value) -> Result<Value> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'name' param"))?;
    manager.enable(name).await?;
    Ok(json!({ "ok": true, "name": name }))
}

/// `plugin.disable` — disable a plugin by name.
pub async fn disable(manager: Arc<PluginManager>, params: Value) -> Result<Value> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'name' param"))?;
    manager.disable(name).await?;
    Ok(json!({ "ok": true, "name": name }))
}

/// `plugin.info` — get detail for a single plugin.
pub async fn info(manager: Arc<PluginManager>, params: Value) -> Result<Value> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'name' param"))?;
    let plugins = manager.list().await;
    let found = plugins.into_iter().find(|p| p.name == name);
    match found {
        Some(p) => Ok(serde_json::to_value(p)?),
        None => anyhow::bail!("plugin '{}' not found", name),
    }
}
