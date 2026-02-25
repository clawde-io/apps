// SPDX-License-Identifier: MIT
// Sprint M: Pack Marketplace — SQLite persistence (PK.T03)
//
// PackStorage wraps the shared SqlitePool for CRUD operations on
// the `installed_packs` table created in migration 024.

use crate::packs::model::InstalledPack;
use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Data-access layer for installed packs.
pub struct PackStorage {
    pool: SqlitePool,
}

impl PackStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ─── Read ─────────────────────────────────────────────────────────────────

    /// Return every pack currently recorded in `installed_packs`.
    pub async fn list_installed(&self) -> Result<Vec<InstalledPack>> {
        let rows = sqlx::query_as::<_, InstalledPack>(
            "SELECT id, name, version, pack_type, publisher, description, \
             install_path, signature, installed_at \
             FROM installed_packs ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Fetch a single pack by name.  Returns `None` if not installed.
    pub async fn get_installed(&self, name: &str) -> Result<Option<InstalledPack>> {
        let row = sqlx::query_as::<_, InstalledPack>(
            "SELECT id, name, version, pack_type, publisher, description, \
             install_path, signature, installed_at \
             FROM installed_packs WHERE name = ?",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    // ─── Write ────────────────────────────────────────────────────────────────

    /// Insert (or replace on name conflict) an installed pack record.
    ///
    /// Called by the installer after all files have been written to disk.
    pub async fn add_installed(&self, pack: &InstalledPack) -> Result<()> {
        sqlx::query(
            "INSERT INTO installed_packs \
             (id, name, version, pack_type, publisher, description, install_path, signature, installed_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(name) DO UPDATE SET \
               id           = excluded.id, \
               version      = excluded.version, \
               pack_type    = excluded.pack_type, \
               publisher    = excluded.publisher, \
               description  = excluded.description, \
               install_path = excluded.install_path, \
               signature    = excluded.signature, \
               installed_at = excluded.installed_at",
        )
        .bind(&pack.id)
        .bind(&pack.name)
        .bind(&pack.version)
        .bind(&pack.pack_type)
        .bind(&pack.publisher)
        .bind(&pack.description)
        .bind(&pack.install_path)
        .bind(&pack.signature)
        .bind(&pack.installed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove the pack record for `name`.  The caller is responsible for
    /// deleting the on-disk files before or after calling this.
    pub async fn remove_installed(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM installed_packs WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ─── Helpers ──────────────────────────────────────────────────────────────

    /// Build a new `InstalledPack` from the given fields and generate a UUID.
    ///
    /// Convenience constructor so callers don't import Uuid/Utc directly.
    pub fn new_pack(
        name: &str,
        version: &str,
        pack_type: &str,
        publisher: Option<&str>,
        description: Option<&str>,
        install_path: &str,
        signature: Option<&str>,
    ) -> InstalledPack {
        InstalledPack {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            version: version.to_string(),
            pack_type: pack_type.to_string(),
            publisher: publisher.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            install_path: install_path.to_string(),
            signature: signature.map(|s| s.to_string()),
            installed_at: Utc::now().to_rfc3339(),
        }
    }
}
