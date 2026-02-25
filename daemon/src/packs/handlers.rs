// SPDX-License-Identifier: MIT
// Sprint M: Pack Marketplace — JSON-RPC 2.0 handlers (PK.T03, PK.T04, PK.T09)
//
// Methods:
//   pack.install          — install from registry or local path
//   pack.update           — reinstall at a newer version
//   pack.remove           — uninstall a pack
//   pack.search           — query the registry (stubbed)
//   pack.listInstalled    — return all installed packs

use crate::packs::{installer::PackInstaller, model::PackSearchResult, storage::PackStorage};
use crate::AppContext;
use anyhow::Result;
use serde_json::Value;
use tracing::debug;

// ─── pack.install ─────────────────────────────────────────────────────────────

/// Install a pack from the registry or a local directory.
///
/// Params:
/// ```json
/// { "name": "clawde-rust-standards" }               // from registry, latest
/// { "name": "my-pack", "version": "0.2.0" }         // from registry, pinned
/// { "local_path": "/path/to/my-pack" }              // from local directory
/// ```
///
/// Returns: `InstalledPack` JSON object.
pub async fn pack_install(params: Value, ctx: &AppContext) -> Result<Value> {
    let data_dir = ctx.config.data_dir.clone();
    let pool = ctx.storage.pool();
    let storage = PackStorage::new(pool);
    let installer = PackInstaller::new(storage);

    if let Some(local_path) = params.get("local_path").and_then(|v| v.as_str()) {
        debug!(local_path, "pack.install (local)");
        let path = std::path::Path::new(local_path);
        let pack = installer.install_local(path, &data_dir).await?;
        return Ok(serde_json::to_value(pack)?);
    }

    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required param: name"))?;

    let version = params.get("version").and_then(|v| v.as_str());

    debug!(name, version, "pack.install (registry)");
    let pack = installer
        .install_from_registry(name, version, &data_dir)
        .await?;
    Ok(serde_json::to_value(pack)?)
}

// ─── pack.update ──────────────────────────────────────────────────────────────

/// Update an installed pack to the latest compatible version.
///
/// Params: `{ "name": "clawde-rust-standards" }`
///
/// The update is implemented as a remove + re-install at `"latest"`.
/// Returns: the updated `InstalledPack` JSON object.
pub async fn pack_update(params: Value, ctx: &AppContext) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required param: name"))?;

    debug!(name, "pack.update");

    let data_dir = ctx.config.data_dir.clone();
    let pool = ctx.storage.pool();
    let storage = PackStorage::new(pool);
    let installer = PackInstaller::new(storage);

    // Remove old version (also cleans up orphaned files).
    installer.remove(name, &data_dir).await?;

    // Re-install at latest.
    let pack = installer
        .install_from_registry(name, None, &data_dir)
        .await?;
    Ok(serde_json::to_value(pack)?)
}

// ─── pack.remove ──────────────────────────────────────────────────────────────

/// Uninstall a pack and delete its on-disk files.
///
/// Params: `{ "name": "clawde-rust-standards" }`
///
/// Returns: `{ "removed": true, "name": "..." }`
pub async fn pack_remove(params: Value, ctx: &AppContext) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required param: name"))?;

    debug!(name, "pack.remove");

    let data_dir = ctx.config.data_dir.clone();
    let pool = ctx.storage.pool();
    let storage = PackStorage::new(pool);
    let installer = PackInstaller::new(storage);

    installer.remove(name, &data_dir).await?;

    Ok(serde_json::json!({ "removed": true, "name": name }))
}

// ─── pack.search ──────────────────────────────────────────────────────────────

/// Query the ClawDE pack registry for packs matching `query`.
///
/// Params: `{ "query": "rust standards" }`
///
/// **Stub:** Returns a hardcoded first-party pack list until the registry
/// backend (PK.T07/T08) is live.
///
/// Returns: `{ "results": [PackSearchResult, ...] }`
pub async fn pack_search(params: Value, _ctx: &AppContext) -> Result<Value> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    debug!(query, "pack.search (registry stub)");

    // TODO(registry): Replace with real HTTP GET to
    //   `https://registry.clawde.io/packs/search?q={query}`
    //   and deserialize the response into Vec<PackSearchResult>.

    let all_packs: Vec<PackSearchResult> = vec![
        PackSearchResult {
            name: "clawde-gci-base".to_string(),
            description: Some("Starter GCI template for new ClawDE projects".to_string()),
            version: "0.1.0".to_string(),
            pack_type: "templates".to_string(),
            downloads: 1_200,
            publisher: Some("clawde-io".to_string()),
        },
        PackSearchResult {
            name: "clawde-rust-standards".to_string(),
            description: Some("Rust coding standards and linting rules".to_string()),
            version: "0.1.0".to_string(),
            pack_type: "rules".to_string(),
            downloads: 980,
            publisher: Some("clawde-io".to_string()),
        },
        PackSearchResult {
            name: "clawde-ts-standards".to_string(),
            description: Some("TypeScript/JavaScript coding standards".to_string()),
            version: "0.1.0".to_string(),
            pack_type: "rules".to_string(),
            downloads: 860,
            publisher: Some("clawde-io".to_string()),
        },
        PackSearchResult {
            name: "clawde-flutter-standards".to_string(),
            description: Some("Flutter/Dart coding standards and patterns".to_string()),
            version: "0.1.0".to_string(),
            pack_type: "rules".to_string(),
            downloads: 740,
            publisher: Some("clawde-io".to_string()),
        },
        PackSearchResult {
            name: "clawde-security-scanner".to_string(),
            description: Some("Security validator — detects common vulnerabilities".to_string()),
            version: "0.1.0".to_string(),
            pack_type: "validators".to_string(),
            downloads: 620,
            publisher: Some("clawde-io".to_string()),
        },
    ];

    // Filter by query (case-insensitive substring match on name + description).
    let results: Vec<&PackSearchResult> = if query.is_empty() {
        all_packs.iter().collect()
    } else {
        all_packs
            .iter()
            .filter(|p| {
                p.name.contains(&query)
                    || p.description
                        .as_deref()
                        .map(|d| d.to_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .collect()
    };

    let count = results.len();
    let results_json = serde_json::to_value(&results)?;

    Ok(serde_json::json!({
        "results": results_json,
        "count": count,
        "query": query,
        "source": "stub",
    }))
}

// ─── pack.listInstalled ───────────────────────────────────────────────────────

/// Return all packs currently installed on this daemon.
///
/// Params: none
///
/// Returns: `{ "packs": [InstalledPack, ...], "count": N }`
pub async fn pack_list_installed(_params: Value, ctx: &AppContext) -> Result<Value> {
    debug!("pack.listInstalled");

    let pool = ctx.storage.pool();
    let storage = PackStorage::new(pool);
    let packs = storage.list_installed().await?;

    let count = packs.len();
    let packs_json = serde_json::to_value(&packs)?;

    Ok(serde_json::json!({
        "packs": packs_json,
        "count": count,
    }))
}
