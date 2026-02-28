// SPDX-License-Identifier: MIT
// Sprint M: Pack Marketplace — JSON-RPC 2.0 handlers (PK.T03, PK.T04, PK.T09, REGISTRY.13-14)
//
// Methods:
//   pack.install          — install from registry or local path
//   pack.update           — reinstall at a newer version
//   pack.remove           — uninstall a pack
//   pack.search           — query the registry (real HTTP)
//   pack.publish          — publish a pack to the registry
//   pack.listInstalled    — return all installed packs

use crate::packs::{installer::PackInstaller, model::PackSearchResult, storage::PackStorage};
use crate::AppContext;
use anyhow::{Context as _, Result};
use serde_json::Value;
use tracing::debug;

// ─── Helper ───────────────────────────────────────────────────────────────────

fn make_installer(ctx: &AppContext) -> PackInstaller {
    let pool = ctx.storage.clone_pool();
    let storage = PackStorage::new(pool);
    PackInstaller::new(storage, &ctx.config.registry_url)
}

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
    let installer = make_installer(ctx);

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
    let installer = make_installer(ctx);

    installer.remove(name, &data_dir).await?;
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
    let installer = make_installer(ctx);
    installer.remove(name, &data_dir).await?;

    Ok(serde_json::json!({ "removed": true, "name": name }))
}

// ─── pack.search ──────────────────────────────────────────────────────────────

/// Query the ClawDE pack registry for packs matching `query`.
///
/// Params: `{ "query": "rust standards" }`
///
/// Makes a real HTTP GET to `{registry_url}/v1/packs?q={query}`.
///
/// Returns: `{ "results": [PackSearchResult, ...], "count": N, "query": "..." }`
pub async fn pack_search(params: Value, ctx: &AppContext) -> Result<Value> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    debug!(query, "pack.search");

    let registry_url = &ctx.config.registry_url;
    let url = format!("{}/v1/packs", registry_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut req = client.get(&url);
    if !query.is_empty() {
        req = req.query(&[("q", &query)]);
    }

    let resp = req.send().await;

    match resp {
        Err(e) => {
            tracing::warn!(err = %e, "registry search request failed — returning empty");
            Ok(serde_json::json!({
                "results": [],
                "count": 0,
                "query": query,
                "error": format!("registry unavailable: {e}"),
            }))
        }
        Ok(r) if !r.status().is_success() => {
            let status = r.status().as_u16();
            tracing::warn!(status, "registry search returned non-200");
            Ok(serde_json::json!({
                "results": [],
                "count": 0,
                "query": query,
                "error": format!("registry returned {status}"),
            }))
        }
        Ok(r) => {
            #[derive(serde::Deserialize)]
            struct RegistrySearchResponse {
                results: Vec<PackSearchResult>,
            }

            match r.json::<RegistrySearchResponse>().await {
                Ok(body) => {
                    let count = body.results.len();
                    Ok(serde_json::json!({
                        "results": body.results,
                        "count": count,
                        "query": query,
                    }))
                }
                Err(e) => {
                    tracing::warn!(err = %e, "failed to parse registry search response");
                    Ok(serde_json::json!({
                        "results": [],
                        "count": 0,
                        "query": query,
                        "error": format!("invalid registry response: {e}"),
                    }))
                }
            }
        }
    }
}

// ─── pack.publish ─────────────────────────────────────────────────────────────

/// Publish a pack from a local directory to the ClawDE registry.
///
/// Params:
/// ```json
/// {
///   "pack_path": "/path/to/my-pack",
///   "token": "pub_abc123..."
/// }
/// ```
///
/// 1. Reads `pack.toml` from `pack_path`
/// 2. Creates a `.tar.gz` of the pack directory in a temp file
/// 3. Posts to `{registry_url}/v1/packs/publish` with Bearer token auth
/// 4. Returns registry response (includes download URL and version)
pub async fn pack_publish(params: Value, ctx: &AppContext) -> Result<Value> {
    let pack_path = params
        .get("pack_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required param: pack_path"))?;

    let token = params
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required param: token"))?;

    let pack_dir = std::path::Path::new(pack_path);
    let manifest = crate::packs::model::PackManifest::load_from_dir(pack_dir)
        .with_context(|| format!("failed to load pack.toml from {pack_path}"))?;

    debug!(name = %manifest.name, version = %manifest.version, "pack.publish");

    // Create a gzipped tarball of the pack directory in a temp file.
    let tarball = build_tarball(pack_dir, &manifest.name, &manifest.version)
        .with_context(|| format!("failed to create tarball for pack '{}'", manifest.name))?;

    let registry_url = &ctx.config.registry_url;
    let url = format!("{}/v1/packs/publish", registry_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let resp = client
        .post(&url)
        .bearer_auth(token)
        .header("Content-Type", "application/octet-stream")
        .header("X-Pack-Name", &manifest.name)
        .header("X-Pack-Version", &manifest.version)
        .header("X-Pack-Type", manifest.pack_type.as_str())
        .body(tarball)
        .send()
        .await
        .with_context(|| format!("registry publish request failed for '{}'", manifest.name))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "registry returned {status} for publish of '{}@{}': {body}",
            manifest.name,
            manifest.version
        );
    }

    let result: Value = resp
        .json()
        .await
        .context("failed to parse registry publish response")?;

    tracing::info!(name = %manifest.name, version = %manifest.version, "pack published to registry");
    Ok(result)
}

// ─── pack.listInstalled ───────────────────────────────────────────────────────

/// Return all packs currently installed on this daemon.
///
/// Params: none
///
/// Returns: `{ "packs": [InstalledPack, ...], "count": N }`
pub async fn pack_list_installed(_params: Value, ctx: &AppContext) -> Result<Value> {
    debug!("pack.listInstalled");

    let pool = ctx.storage.clone_pool();
    let storage = PackStorage::new(pool);
    let packs = storage.list_installed().await?;

    let count = packs.len();
    let packs_json = serde_json::to_value(&packs)?;

    Ok(serde_json::json!({
        "packs": packs_json,
        "count": count,
    }))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Build a gzipped tar archive from a pack directory.
///
/// The archive uses `{name}-{version}/` as the top-level prefix so extraction
/// produces a self-contained directory.
fn build_tarball(pack_dir: &std::path::Path, name: &str, version: &str) -> Result<Vec<u8>> {
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write as _;
    use tar::Builder;

    let mut buf = Vec::new();
    {
        let enc = GzEncoder::new(&mut buf, Compression::default());
        let mut archive = Builder::new(enc);

        let prefix = format!("{name}-{version}");
        archive.append_dir_all(&prefix, pack_dir).with_context(|| {
            format!(
                "failed to add pack directory {} to archive",
                pack_dir.display()
            )
        })?;

        archive
            .into_inner()
            .context("failed to finalise tar archive")?
            .finish()
            .context("failed to finalise gzip stream")?
            .flush()
            .context("failed to flush gzip output")?;
    }

    Ok(buf)
}
