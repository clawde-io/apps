//! Sprint M: Pack Marketplace integration tests.
//!
//! Tests cover:
//!   - PackStorage install + list + remove cycle
//!   - PackManifest deserialization from a pack.toml
//!   - PackInstaller local install creates the expected directory structure
//!   - PackSigner stub sign + verify round-trip

use clawd::packs::{
    installer::PackInstaller,
    model::{PackManifest, PackType},
    signing::PackSigner,
    storage::PackStorage,
};
use sqlx::SqlitePool;
use std::path::PathBuf;
use tempfile::TempDir;

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Create an in-memory (temp file) SQLite pool with the full migration set.
async fn make_pool(dir: &TempDir) -> SqlitePool {
    let db_path = dir.path().join("clawd_test.db");
    let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await
        .expect("open test db");
    sqlx::migrate!("src/storage/migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    pool
}

/// Create a minimal pack directory with a valid `pack.toml`.
fn make_local_pack(dir: &TempDir, name: &str, version: &str, pack_type: &str) -> PathBuf {
    let pack_dir = dir.path().join(name);
    std::fs::create_dir_all(&pack_dir).unwrap();

    let manifest_toml = format!(
        r#"name = "{name}"
version = "{version}"
type = "{pack_type}"
description = "Test pack"
publisher = "test-publisher"
files = ["rules/example.md"]
"#
    );
    std::fs::write(pack_dir.join("pack.toml"), &manifest_toml).unwrap();

    // Create a listed file so the installer can copy it.
    std::fs::create_dir_all(pack_dir.join("rules")).unwrap();
    std::fs::write(pack_dir.join("rules").join("example.md"), "# Example rule\n").unwrap();

    pack_dir
}

// ─── PackStorage tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_pack_storage_install_list_remove() {
    let db_dir = TempDir::new().unwrap();
    let pool = make_pool(&db_dir).await;
    let storage = PackStorage::new(pool);

    // Initially empty.
    let packs = storage.list_installed().await.unwrap();
    assert!(packs.is_empty(), "should start with no installed packs");

    // Add a pack.
    let pack = PackStorage::new_pack(
        "clawde-rust-standards",
        "0.1.0",
        "rules",
        Some("clawde-io"),
        Some("Rust coding standards"),
        "/data/packs/clawde-rust-standards-0.1.0",
        None,
    );
    storage.add_installed(&pack).await.unwrap();

    // List returns the pack.
    let packs = storage.list_installed().await.unwrap();
    assert_eq!(packs.len(), 1, "should have one installed pack");
    assert_eq!(packs[0].name, "clawde-rust-standards");
    assert_eq!(packs[0].version, "0.1.0");
    assert_eq!(packs[0].pack_type, "rules");
    assert_eq!(packs[0].publisher.as_deref(), Some("clawde-io"));

    // get_installed by name.
    let fetched = storage
        .get_installed("clawde-rust-standards")
        .await
        .unwrap();
    assert!(fetched.is_some(), "should find pack by name");
    assert_eq!(fetched.unwrap().version, "0.1.0");

    // Non-existent pack returns None.
    let missing = storage.get_installed("does-not-exist").await.unwrap();
    assert!(missing.is_none(), "missing pack should return None");

    // Remove the pack.
    storage
        .remove_installed("clawde-rust-standards")
        .await
        .unwrap();

    let packs_after = storage.list_installed().await.unwrap();
    assert!(packs_after.is_empty(), "pack list should be empty after remove");
}

#[tokio::test]
async fn test_pack_storage_upsert_on_conflict() {
    let db_dir = TempDir::new().unwrap();
    let pool = make_pool(&db_dir).await;
    let storage = PackStorage::new(pool);

    // Install v0.1.0.
    let pack_v1 = PackStorage::new_pack(
        "my-pack",
        "0.1.0",
        "skills",
        None,
        None,
        "/data/packs/my-pack-0.1.0",
        None,
    );
    storage.add_installed(&pack_v1).await.unwrap();

    // "Upgrade" to v0.2.0 using add_installed (ON CONFLICT UPDATE).
    let pack_v2 = PackStorage::new_pack(
        "my-pack",
        "0.2.0",
        "skills",
        None,
        None,
        "/data/packs/my-pack-0.2.0",
        None,
    );
    storage.add_installed(&pack_v2).await.unwrap();

    // Should still be exactly one record, now at v0.2.0.
    let packs = storage.list_installed().await.unwrap();
    assert_eq!(packs.len(), 1, "upsert should not duplicate");
    assert_eq!(packs[0].version, "0.2.0", "version should be updated");
}

// ─── PackManifest tests ───────────────────────────────────────────────────────

#[test]
fn test_pack_manifest_deserialization() {
    let toml_src = r#"
name        = "clawde-ts-standards"
version     = "0.2.1"
type        = "rules"
description = "TypeScript standards"
publisher   = "clawde-io"
files       = ["rules/ts.md", "rules/react.md"]

[[dependencies]]
name    = "clawde-gci-base"
version = "^1.0.0"
"#;

    let manifest: PackManifest = toml::from_str(toml_src).expect("parse pack manifest");

    assert_eq!(manifest.name, "clawde-ts-standards");
    assert_eq!(manifest.version, "0.2.1");
    assert_eq!(manifest.pack_type, PackType::Rules);
    assert_eq!(manifest.publisher.as_deref(), Some("clawde-io"));
    assert_eq!(manifest.files.len(), 2);
    assert_eq!(manifest.dependencies.len(), 1);
    assert_eq!(manifest.dependencies[0].name, "clawde-gci-base");
    assert_eq!(
        manifest.dependencies[0].version.as_deref(),
        Some("^1.0.0")
    );
}

#[test]
fn test_pack_manifest_load_from_dir() {
    let dir = TempDir::new().unwrap();
    let pack_dir = make_local_pack(&dir, "test-pack", "0.1.0", "rules");

    let manifest = PackManifest::load_from_dir(&pack_dir).expect("load manifest");
    assert_eq!(manifest.name, "test-pack");
    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.pack_type, PackType::Rules);
}

#[test]
fn test_pack_manifest_invalid_semver_rejected() {
    let dir = TempDir::new().unwrap();
    let pack_dir = dir.path().join("bad-pack");
    std::fs::create_dir_all(&pack_dir).unwrap();
    std::fs::write(
        pack_dir.join("pack.toml"),
        r#"name = "bad-pack"\nversion = "not-semver"\ntype = "rules"\n"#,
    )
    .unwrap();

    // Should fail — "not-semver" is not valid semver.
    let result = PackManifest::load_from_dir(&pack_dir);
    assert!(result.is_err(), "invalid semver should be rejected");
}

// ─── PackInstaller tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_installer_local_creates_directory_structure() {
    let db_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let pack_source_dir = TempDir::new().unwrap();

    let pool = make_pool(&db_dir).await;
    let storage = PackStorage::new(pool);
    let installer = PackInstaller::new(storage);

    // Create a minimal local pack.
    let pack_dir = make_local_pack(&pack_source_dir, "my-local-pack", "1.0.0", "rules");

    let installed = installer
        .install_local(&pack_dir, data_dir.path())
        .await
        .expect("install local pack");

    assert_eq!(installed.name, "my-local-pack");
    assert_eq!(installed.version, "1.0.0");
    assert_eq!(installed.pack_type, "rules");

    // Check the expected on-disk layout.
    let expected_dir = data_dir.path().join("packs").join("my-local-pack-1.0.0");
    assert!(expected_dir.exists(), "install directory should be created");
    assert!(
        expected_dir.join("pack.toml").exists(),
        "pack.toml should be copied"
    );
    assert!(
        expected_dir.join("rules").join("example.md").exists(),
        "listed content file should be copied"
    );
}

#[tokio::test]
async fn test_installer_remove_deletes_files_and_record() {
    let db_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let pack_source_dir = TempDir::new().unwrap();

    let pool = make_pool(&db_dir).await;
    let storage_for_check = PackStorage::new(pool.clone());
    let storage = PackStorage::new(pool);
    let installer = PackInstaller::new(storage);

    let pack_dir = make_local_pack(&pack_source_dir, "removable-pack", "0.1.0", "agents");
    installer
        .install_local(&pack_dir, data_dir.path())
        .await
        .expect("install");

    // Verify it's recorded.
    let before = storage_for_check
        .get_installed("removable-pack")
        .await
        .unwrap();
    assert!(before.is_some(), "pack should be installed");

    // Remove it.
    installer
        .remove("removable-pack", data_dir.path())
        .await
        .expect("remove");

    // Record should be gone.
    let after = storage_for_check
        .get_installed("removable-pack")
        .await
        .unwrap();
    assert!(after.is_none(), "record should be removed from DB");

    // Files should be gone.
    let install_dir = data_dir
        .path()
        .join("packs")
        .join("removable-pack-0.1.0");
    assert!(!install_dir.exists(), "install directory should be deleted");
}

// ─── PackSigner tests ─────────────────────────────────────────────────────────

#[test]
fn test_signer_stub_sign_and_verify() {
    let dir = TempDir::new().unwrap();
    let pack_dir = make_local_pack(&dir, "signed-pack", "0.1.0", "skills");

    // Sign (stub — no real key needed).
    let fake_key = dir.path().join("fake.key");
    std::fs::write(&fake_key, b"not-a-real-key").unwrap();

    let sig = PackSigner::sign_pack(&pack_dir, &fake_key).expect("sign pack");
    assert!(sig.starts_with("stub-sig:"), "stub signature should have prefix");

    // Verify the stub signature.
    let valid = PackSigner::verify_signature(&pack_dir, &sig, "any-public-key").expect("verify");
    assert!(valid, "stub signature should verify correctly");

    // Verify a wrong signature fails.
    let invalid = PackSigner::verify_signature(&pack_dir, "bad-sig", "any-public-key").expect("verify");
    assert!(!invalid, "bad signature should not verify");
}
