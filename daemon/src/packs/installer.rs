// SPDX-License-Identifier: MIT
// Sprint M: Pack Marketplace — installer (PK.T03, PK.T04, PK.T06)
//
// PackInstaller handles fetching from the registry (stubbed), copying local
// pack directories, and removing packs from disk.

use crate::packs::model::{InstalledPack, PackManifest};
use crate::packs::storage::PackStorage;
use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Registry base URL for fetching packs.
///
/// Actual HTTP fetch is stubbed — the URL is constructed here so the wiring
/// is clear for the real implementation in Sprint M.2 (PK.T07/T08).
const REGISTRY_BASE: &str = "https://registry.clawde.io/packs";

/// Manages installation, updating, and removal of packs on disk.
pub struct PackInstaller {
    storage: PackStorage,
}

impl PackInstaller {
    pub fn new(storage: PackStorage) -> Self {
        Self { storage }
    }

    // ─── Registry install ─────────────────────────────────────────────────────

    /// Install a pack from the ClawDE registry.
    ///
    /// When `version` is `None` the installer resolves to `"latest"`.
    ///
    /// **Stub:** The actual HTTP download is not yet implemented.  The method
    /// constructs the target install path as `{data_dir}/packs/{name}-{version}/`
    /// and records the pack in the database so the rest of the lifecycle works
    /// end-to-end once the registry is available.  A real download will replace
    /// the stub block marked with `// TODO(registry)`.
    pub async fn install_from_registry(
        &self,
        name: &str,
        version: Option<&str>,
        data_dir: &Path,
    ) -> Result<InstalledPack> {
        let resolved_version = version.unwrap_or("latest");
        debug!(name, version = resolved_version, "pack.install from registry");

        // TODO(registry): Replace this stub with an actual HTTP GET to
        //   `{REGISTRY_BASE}/{name}/{resolved_version}`
        //   Unpack the tarball / zip into `install_path`.
        //   Verify the bundle signature before writing any files.
        let _registry_url = format!("{REGISTRY_BASE}/{name}/{resolved_version}");

        let install_path = data_dir
            .join("packs")
            .join(format!("{name}-{resolved_version}"));

        // Ensure the directory exists so subsequent operations are safe.
        tokio::fs::create_dir_all(&install_path).await.with_context(|| {
            format!("failed to create install dir {}", install_path.display())
        })?;

        // Write a stub pack.toml so the directory is a valid pack.
        let stub_manifest = format!(
            "[package]\nname = \"{name}\"\nversion = \"{resolved_version}\"\ntype = \"skills\"\n"
        );
        let manifest_path = install_path.join("pack.toml");
        if !manifest_path.exists() {
            tokio::fs::write(&manifest_path, stub_manifest)
                .await
                .with_context(|| format!("failed to write stub manifest at {}", manifest_path.display()))?;
        }

        let pack = PackStorage::new_pack(
            name,
            resolved_version,
            "skills",
            None,
            None,
            &install_path.to_string_lossy(),
            None,
        );

        self.storage.add_installed(&pack).await?;
        info!(name, version = resolved_version, "pack installed (registry stub)");
        Ok(pack)
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
            tokio::fs::remove_dir_all(&install_path).await.with_context(|| {
                format!("failed to remove pack files at {}", install_path.display())
            })?;
        } else {
            // Files may have been deleted manually — log and continue.
            tracing::warn!(path = %install_path.display(), "pack directory not found on disk during remove");
        }

        self.storage.remove_installed(name).await?;

        // Clean up empty parent `packs/` directory if it's now empty.
        let packs_dir = data_dir.join("packs");
        let _ = tokio::fs::remove_dir(&packs_dir).await; // silently ignore if not empty

        info!(name, "pack removed");
        Ok(())
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Copy pack content files from `src_dir` into `dest_dir`.
///
/// The function copies `pack.toml` plus every file path listed in
/// `manifest.files`.  Glob expansion is not yet implemented — entries are
/// treated as literal relative paths.  If `manifest.files` is empty, the
/// entire source directory is mirrored.
async fn copy_pack_files(src_dir: &Path, dest_dir: &Path, manifest: &PackManifest) -> Result<()> {
    tokio::fs::create_dir_all(dest_dir).await.with_context(|| {
        format!("failed to create pack dest dir {}", dest_dir.display())
    })?;

    // Always copy the manifest itself.
    let src_manifest = src_dir.join("pack.toml");
    let dst_manifest = dest_dir.join("pack.toml");
    tokio::fs::copy(&src_manifest, &dst_manifest)
        .await
        .with_context(|| format!("failed to copy pack.toml from {}", src_manifest.display()))?;

    if manifest.files.is_empty() {
        // Mirror the whole source directory (except pack.toml already copied).
        copy_dir_recursive(src_dir, dest_dir).await?;
    } else {
        for file_path in &manifest.files {
            let src = src_dir.join(file_path);
            let dst = dest_dir.join(file_path);

            if let Some(parent) = dst.parent() {
                tokio::fs::create_dir_all(parent).await.with_context(|| {
                    format!("failed to create parent dir {}", parent.display())
                })?;
            }

            if src.is_file() {
                tokio::fs::copy(&src, &dst).await.with_context(|| {
                    format!("failed to copy pack file {}", src.display())
                })?;
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
        // pack.toml is already copied by the caller.
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

/// Boxed wrapper for `copy_dir_recursive` to allow async recursion without
/// triggering the "recursive async fn" compiler error.
fn copy_dir_recursive_boxed(
    src: PathBuf,
    dst: PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> {
    Box::pin(async move { copy_dir_recursive(&src, &dst).await })
}
