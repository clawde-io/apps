// SPDX-License-Identifier: MIT
// Sprint M: Pack Marketplace — installer (PK.T03, PK.T04, PK.T06, REGISTRY.12)
//
// PackInstaller handles fetching from the registry (real HTTP), copying local
// pack directories, and removing packs from disk.

use crate::packs::model::{InstalledPack, PackManifest};
use crate::packs::storage::PackStorage;
use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Manages installation, updating, and removal of packs on disk.
pub struct PackInstaller {
    storage: PackStorage,
    /// Registry base URL — typically `https://registry.clawde.io`.
    /// Configurable via `CLAWD_REGISTRY_URL` env var or config.toml.
    registry_url: String,
}

impl PackInstaller {
    pub fn new(storage: PackStorage, registry_url: impl Into<String>) -> Self {
        Self {
            storage,
            registry_url: registry_url.into(),
        }
    }

    // ─── Registry install ─────────────────────────────────────────────────────

    /// Install a pack from the ClawDE registry.
    ///
    /// 1. Fetches `{registry_url}/v1/packs/{name}/{version}.tar.gz`
    /// 2. Verifies the SHA-256 digest from the `X-Pack-SHA256` response header
    /// 3. Optionally verifies the ed25519 signature from `X-Pack-Signature` and
    ///    `X-Pack-Publisher-Key` headers (skipped if either header is absent)
    /// 4. Extracts the tarball into `{data_dir}/packs/{name}-{version}/`
    /// 5. Reads `pack.toml` from the extracted directory
    /// 6. Records the pack in the database
    pub async fn install_from_registry(
        &self,
        name: &str,
        version: Option<&str>,
        data_dir: &Path,
    ) -> Result<InstalledPack> {
        let resolved_version = version.unwrap_or("latest");
        debug!(
            name,
            version = resolved_version,
            "pack.install from registry"
        );

        // Download the tarball from the registry.
        let (tarball_bytes, expected_sha256, signature, publisher_key) =
            self.download_tarball(name, resolved_version).await?;

        // Verify SHA-256 digest.
        let actual_sha256 = hex::encode(sha256_bytes(&tarball_bytes));
        if let Some(expected) = &expected_sha256 {
            if actual_sha256 != expected.to_lowercase() {
                anyhow::bail!(
                    "pack SHA-256 mismatch for '{name}@{resolved_version}': \
                     expected {expected}, got {actual_sha256}"
                );
            }
        } else {
            warn!(
                name,
                version = resolved_version,
                "registry did not provide X-Pack-SHA256 header — skipping digest check"
            );
        }

        // Build install path.
        let install_path = data_dir
            .join("packs")
            .join(format!("{name}-{resolved_version}"));

        tokio::fs::create_dir_all(&install_path)
            .await
            .with_context(|| format!("failed to create install dir {}", install_path.display()))?;

        // Extract the tarball.
        extract_tarball(&tarball_bytes, &install_path)?;

        // Verify signature over the extracted pack.toml (if provided).
        if let (Some(sig), Some(pub_key)) = (signature, publisher_key) {
            use crate::packs::signing::PackSigner;
            match PackSigner::verify_signature(&install_path, &sig, &pub_key) {
                Ok(true) => {
                    debug!(name, version = resolved_version, "pack signature verified");
                }
                Ok(false) => {
                    // Clean up before bailing.
                    let _ = tokio::fs::remove_dir_all(&install_path).await;
                    anyhow::bail!(
                        "pack signature verification failed for '{name}@{resolved_version}': \
                         signature does not match publisher key"
                    );
                }
                Err(e) => {
                    warn!(name, version = resolved_version, err = %e, "signature verification error — continuing");
                }
            }
        }

        // Read the extracted manifest.
        let manifest = PackManifest::load_from_dir(&install_path).with_context(|| {
            format!("failed to load manifest for '{name}@{resolved_version}' after extraction")
        })?;

        // Determine the actual version from the manifest (in case "latest" was resolved).
        let actual_version = manifest.version.clone();

        let pack = PackStorage::new_pack(
            &manifest.name,
            &actual_version,
            manifest.pack_type.as_str(),
            manifest.publisher.as_deref(),
            manifest.description.as_deref(),
            &install_path.to_string_lossy(),
            None,
        );

        self.storage.add_installed(&pack).await?;
        info!(name, version = actual_version, "pack installed (registry)");
        Ok(pack)
    }

    /// Download the pack tarball from the registry.
    ///
    /// Returns `(bytes, sha256_header, signature_header, publisher_key_header)`.
    async fn download_tarball(
        &self,
        name: &str,
        version: &str,
    ) -> Result<(Vec<u8>, Option<String>, Option<String>, Option<String>)> {
        let url = format!("{}/v1/packs/{}/{}.tar.gz", self.registry_url, name, version);
        debug!(url, "fetching pack tarball");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .context("failed to build HTTP client")?;

        let resp = client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("registry request failed for {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("registry returned {status} for pack '{name}@{version}': {body}");
        }

        let sha256 = resp
            .headers()
            .get("x-pack-sha256")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let signature = resp
            .headers()
            .get("x-pack-signature")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let publisher_key = resp
            .headers()
            .get("x-pack-publisher-key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("failed to read tarball bytes for '{name}@{version}'"))?;

        Ok((bytes.to_vec(), sha256, signature, publisher_key))
    }

    // ─── Local install ────────────────────────────────────────────────────────

    /// Install a pack from a local directory.
    ///
    /// Reads `pack.toml`, copies all listed files into
    /// `{data_dir}/packs/{name}-{version}/`, and records the result in the
    /// database.
    pub async fn install_local(&self, pack_dir: &Path, data_dir: &Path) -> Result<InstalledPack> {
        debug!(pack_dir = %pack_dir.display(), "pack.install local");

        let manifest = PackManifest::load_from_dir(pack_dir)
            .with_context(|| format!("failed to load manifest from {}", pack_dir.display()))?;

        let install_path = data_dir
            .join("packs")
            .join(format!("{}-{}", manifest.name, manifest.version));

        copy_pack_files(pack_dir, &install_path, &manifest).await?;

        let pack = PackStorage::new_pack(
            &manifest.name,
            &manifest.version,
            manifest.pack_type.as_str(),
            manifest.publisher.as_deref(),
            manifest.description.as_deref(),
            &install_path.to_string_lossy(),
            None,
        );

        self.storage.add_installed(&pack).await?;
        info!(name = %manifest.name, version = %manifest.version, "pack installed (local)");
        Ok(pack)
    }

    // ─── Remove ───────────────────────────────────────────────────────────────

    /// Remove a pack: delete its on-disk files and remove the database record.
    pub async fn remove(&self, name: &str, data_dir: &Path) -> Result<()> {
        debug!(name, "pack.remove");

        let record = self
            .storage
            .get_installed(name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("pack '{}' is not installed", name))?;

        // Remove files first so that a DB failure doesn't leave orphaned files
        // that can't be cleaned up via the API.
        let install_path = PathBuf::from(&record.install_path);
        if install_path.exists() {
            tokio::fs::remove_dir_all(&install_path)
                .await
                .with_context(|| {
                    format!("failed to remove pack files at {}", install_path.display())
                })?;
        } else {
            tracing::warn!(path = %install_path.display(), "pack directory not found on disk during remove");
        }

        self.storage.remove_installed(name).await?;

        // Clean up empty parent `packs/` directory if it's now empty.
        let packs_dir = data_dir.join("packs");
        let _ = tokio::fs::remove_dir(&packs_dir).await;

        info!(name, "pack removed");
        Ok(())
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Compute SHA-256 digest of a byte slice.
fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Decompress and extract a `.tar.gz` archive into `dest_dir`.
fn extract_tarball(tarball_bytes: &[u8], dest_dir: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let gz = GzDecoder::new(tarball_bytes);
    let mut archive = Archive::new(gz);

    // Strip the top-level directory component that tarballs typically include.
    for entry in archive
        .entries()
        .context("failed to read tarball entries")?
    {
        let mut entry = entry.context("corrupt tarball entry")?;
        let entry_path = entry.path().context("invalid path in tarball")?;

        // Strip the first path component (e.g., "pack-0.1.0/") if present.
        let stripped: PathBuf = entry_path.components().skip(1).collect();
        if stripped.as_os_str().is_empty() {
            continue;
        }

        // Security: reject absolute paths and path traversal attempts.
        if stripped.is_absolute() || stripped.components().any(|c| c.as_os_str() == "..") {
            warn!(path = %stripped.display(), "rejected unsafe path in pack tarball");
            continue;
        }

        let out_path = dest_dir.join(&stripped);
        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&out_path)
                .with_context(|| format!("failed to create dir {}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create parent {}", parent.display()))?;
            }
            entry
                .unpack(&out_path)
                .with_context(|| format!("failed to extract {}", out_path.display()))?;
        }
    }

    Ok(())
}

/// Copy pack content files from `src_dir` into `dest_dir`.
///
/// The function copies `pack.toml` plus every file path listed in
/// `manifest.files`.  If `manifest.files` is empty, the entire source
/// directory is mirrored.
async fn copy_pack_files(src_dir: &Path, dest_dir: &Path, manifest: &PackManifest) -> Result<()> {
    tokio::fs::create_dir_all(dest_dir)
        .await
        .with_context(|| format!("failed to create pack dest dir {}", dest_dir.display()))?;

    // Always copy the manifest itself.
    let src_manifest = src_dir.join("pack.toml");
    let dst_manifest = dest_dir.join("pack.toml");
    tokio::fs::copy(&src_manifest, &dst_manifest)
        .await
        .with_context(|| format!("failed to copy pack.toml from {}", src_manifest.display()))?;

    if manifest.files.is_empty() {
        copy_dir_recursive(src_dir, dest_dir).await?;
    } else {
        for file_path in &manifest.files {
            let src = src_dir.join(file_path);
            let dst = dest_dir.join(file_path);

            if let Some(parent) = dst.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("failed to create parent dir {}", parent.display()))?;
            }

            if src.is_file() {
                tokio::fs::copy(&src, &dst)
                    .await
                    .with_context(|| format!("failed to copy pack file {}", src.display()))?;
            }
        }
    }

    Ok(())
}

/// Recursively copy all files from `src` into `dst` (skipping `pack.toml`
/// which is always handled first by the caller).
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    let mut entries = tokio::fs::read_dir(src)
        .await
        .with_context(|| format!("cannot read dir {}", src.display()))?;

    while let Some(entry) = entries.next_entry().await? {
        let entry_name = entry.file_name();
        if entry_name == "pack.toml" {
            continue;
        }
        let entry_src = entry.path();
        let entry_dst = dst.join(&entry_name);

        if entry_src.is_dir() {
            tokio::fs::create_dir_all(&entry_dst).await?;
            copy_dir_recursive_boxed(entry_src, entry_dst).await?;
        } else {
            tokio::fs::copy(&entry_src, &entry_dst)
                .await
                .with_context(|| format!("failed to copy {}", entry_src.display()))?;
        }
    }
    Ok(())
}

/// Boxed wrapper for `copy_dir_recursive` to allow async recursion.
fn copy_dir_recursive_boxed(
    src: PathBuf,
    dst: PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> {
    Box::pin(async move { copy_dir_recursive(&src, &dst).await })
}
