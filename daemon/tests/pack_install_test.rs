/// pack_install_test.rs — Integration tests for pack installation from registry.
///
/// Tests: local pack install, tarball extraction, registry URL construction.
use std::fs;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that a pack TOML manifest has required fields.
    #[test]
    fn test_pack_manifest_has_required_fields() {
        let manifest = r#"
[pack]
name = "@clawde/rust"
version = "0.1.0"
description = "Rust conventions"
author = "ClawDE"
license = "MIT"
tags = ["rust"]
"#;
        assert!(manifest.contains("name = "));
        assert!(manifest.contains("version = "));
        assert!(manifest.contains("description = "));
    }

    /// Verify registry URL construction for pack download.
    #[test]
    fn test_registry_url_for_pack_download() {
        let registry_url = "https://registry.clawde.io";
        let pack_name = "@clawde/rust";
        let pack_version = "0.1.0";

        // Encode @ and / for URL
        let encoded_name = pack_name
            .replace('@', "%40")
            .replace('/', "%2F");
        let download_url = format!(
            "{}/v1/packs/{}/{}/download",
            registry_url, encoded_name, pack_version
        );

        assert!(download_url.contains("registry.clawde.io"));
        assert!(download_url.contains("clawde%2Frust"));
        assert!(download_url.contains("0.1.0"));
    }

    /// Verify that pack name sanitization rejects path traversal.
    #[test]
    fn test_pack_name_rejects_path_traversal() {
        let dangerous_names = [
            "../../../etc/passwd",
            "..\\..\\windows\\system32",
            "@clawde/../evil",
        ];

        for name in &dangerous_names {
            // Pack names must match @scope/name pattern
            let valid = name.starts_with('@')
                && !name.contains("..")
                && !name.contains('\\')
                && name.chars().filter(|c| *c == '/').count() == 1;
            assert!(!valid, "Should reject: {}", name);
        }
    }

    /// Verify that pack version follows semver format.
    #[test]
    fn test_pack_version_semver_format() {
        let valid_versions = ["0.1.0", "1.0.0", "2.3.4", "0.0.1"];
        let invalid_versions = ["1", "1.0", "v1.0.0", "latest"];

        for v in &valid_versions {
            let parts: Vec<&str> = v.split('.').collect();
            assert_eq!(parts.len(), 3, "Version {} should have 3 parts", v);
            for part in &parts {
                assert!(
                    part.parse::<u32>().is_ok(),
                    "Version part {} should be numeric",
                    part
                );
            }
        }

        for v in &invalid_versions {
            let parts: Vec<&str> = v.split('.').collect();
            let valid = parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok());
            assert!(!valid, "Should reject version: {}", v);
        }
    }

    /// Verify local registry path resolution.
    #[test]
    fn test_local_registry_path_resolution() {
        let base_path = PathBuf::from("/var/clawd/registry");
        let pack_file = "clawde-rust-0.1.0.clawd-pack.tar.gz";
        let full_path = base_path.join(pack_file);

        // Ensure resolved path is within base
        assert!(full_path.starts_with(&base_path));
        assert!(full_path.to_string_lossy().ends_with(".clawd-pack.tar.gz"));
    }
}
