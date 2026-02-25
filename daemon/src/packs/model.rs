// SPDX-License-Identifier: MIT
// Sprint M: Pack Marketplace — data models (PK.T01)
//
// PackManifest is parsed from pack.toml inside each pack directory.
// InstalledPack is the persisted record in installed_packs.
// PackSearchResult is returned by pack.search (registry query).

use serde::{Deserialize, Serialize};

// ─── PackType ────────────────────────────────────────────────────────────────

/// The functional category of a pack.  Determines where the pack's files
/// are loaded from during daemon startup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackType {
    Skills,
    Rules,
    Agents,
    Validators,
    Templates,
}

impl PackType {
    /// Return the canonical string representation stored in SQLite and the
    /// JSON-RPC API.
    pub fn as_str(&self) -> &'static str {
        match self {
            PackType::Skills => "skills",
            PackType::Rules => "rules",
            PackType::Agents => "agents",
            PackType::Validators => "validators",
            PackType::Templates => "templates",
        }
    }

    /// Parse from a string; returns an error for unknown variants.
    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "skills" => Ok(PackType::Skills),
            "rules" => Ok(PackType::Rules),
            "agents" => Ok(PackType::Agents),
            "validators" => Ok(PackType::Validators),
            "templates" => Ok(PackType::Templates),
            other => Err(anyhow::anyhow!("unknown pack type: {}", other)),
        }
    }
}

// ─── PackDep ─────────────────────────────────────────────────────────────────

/// A single pack dependency declaration (name + optional semver constraint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackDep {
    pub name: String,
    /// Semver constraint string, e.g. `"^1.0.0"` or `">=0.2, <1"`.
    /// `None` means "any version".
    pub version: Option<String>,
}

// ─── PackManifest ─────────────────────────────────────────────────────────────

/// Contents of a `pack.toml` file.
///
/// ```toml
/// name        = "clawde-rust-standards"
/// version     = "0.1.0"
/// type        = "rules"
/// description = "Rust coding standards for ClawDE projects"
/// publisher   = "clawde-io"
/// files       = ["rules/*.md"]
/// [[dependencies]]
/// name    = "clawde-gci-base"
/// version = "^1.0.0"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    pub name: String,
    /// Semver string — validated during install but stored as-is.
    pub version: String,
    /// Pack category.
    #[serde(rename = "type")]
    pub pack_type: PackType,
    /// Human-readable description (optional).
    pub description: Option<String>,
    /// Publisher identifier (optional, e.g. `"clawde-io"` or a GitHub username).
    pub publisher: Option<String>,
    /// Glob patterns or explicit paths that make up the pack's content.
    #[serde(default)]
    pub files: Vec<String>,
    /// Other packs this pack depends on (resolved during install).
    #[serde(default)]
    pub dependencies: Vec<PackDep>,
}

impl PackManifest {
    /// Load and parse a `pack.toml` from the given pack directory.
    pub fn load_from_dir(pack_dir: &std::path::Path) -> anyhow::Result<Self> {
        let manifest_path = pack_dir.join("pack.toml");
        let raw = std::fs::read_to_string(&manifest_path)
            .map_err(|e| anyhow::anyhow!("cannot read pack.toml at {}: {}", manifest_path.display(), e))?;
        let manifest: PackManifest = toml::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("invalid pack.toml: {}", e))?;
        // Validate that the version string is valid semver.
        semver::Version::parse(&manifest.version)
            .map_err(|e| anyhow::anyhow!("pack version '{}' is not valid semver: {}", manifest.version, e))?;
        Ok(manifest)
    }
}

// ─── InstalledPack ────────────────────────────────────────────────────────────

/// A pack that has been installed and is tracked in the SQLite database.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InstalledPack {
    pub id: String,
    pub name: String,
    pub version: String,
    pub pack_type: String,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub install_path: String,
    pub signature: Option<String>,
    pub installed_at: String,
}

// ─── PackSearchResult ─────────────────────────────────────────────────────────

/// A single result item from a registry search query (`pack.search`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSearchResult {
    pub name: String,
    pub description: Option<String>,
    /// Latest version available in the registry.
    pub version: String,
    pub pack_type: String,
    /// Total install/download count (for ranking search results).
    pub downloads: u64,
    pub publisher: Option<String>,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn pack_type_as_str_roundtrip() {
        let cases = [
            (PackType::Skills, "skills"),
            (PackType::Rules, "rules"),
            (PackType::Agents, "agents"),
            (PackType::Validators, "validators"),
            (PackType::Templates, "templates"),
        ];
        for (variant, expected) in cases {
            assert_eq!(variant.as_str(), expected);
        }
    }

    #[test]
    fn pack_type_from_str_roundtrip() {
        for s in ["skills", "rules", "agents", "validators", "templates"] {
            let t = PackType::from_str(s).expect("should parse");
            assert_eq!(t.as_str(), s);
        }
    }

    #[test]
    fn pack_type_from_str_errors_on_unknown() {
        assert!(PackType::from_str("unknown-type").is_err());
        assert!(PackType::from_str("").is_err());
    }

    #[test]
    fn pack_manifest_load_from_dir_parses_valid_toml() {
        let dir = TempDir::new().unwrap();
        let toml = r#"
name        = "test-pack"
version     = "1.2.3"
type        = "rules"
description = "Test pack"
publisher   = "test-user"
files       = ["rules/*.md"]
"#;
        std::fs::write(dir.path().join("pack.toml"), toml).unwrap();
        let manifest = PackManifest::load_from_dir(dir.path()).unwrap();
        assert_eq!(manifest.name, "test-pack");
        assert_eq!(manifest.version, "1.2.3");
        assert_eq!(manifest.pack_type, PackType::Rules);
        assert_eq!(manifest.publisher.as_deref(), Some("test-user"));
    }

    #[test]
    fn pack_manifest_rejects_invalid_semver() {
        let dir = TempDir::new().unwrap();
        let toml = r#"
name    = "bad-version"
version = "not-semver"
type    = "skills"
"#;
        std::fs::write(dir.path().join("pack.toml"), toml).unwrap();
        assert!(PackManifest::load_from_dir(dir.path()).is_err());
    }

    #[test]
    fn pack_manifest_missing_file_errors() {
        let dir = TempDir::new().unwrap();
        assert!(PackManifest::load_from_dir(dir.path()).is_err());
    }

    #[test]
    fn pack_dep_optional_version() {
        let dep = PackDep { name: "some-pack".to_string(), version: None };
        assert!(dep.version.is_none());
        let dep2 = PackDep { name: "other".to_string(), version: Some("^1.0.0".to_string()) };
        assert_eq!(dep2.version.as_deref(), Some("^1.0.0"));
    }
}
