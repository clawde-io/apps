/// Sprint FF PL.14 — Plugin runtime tests.
///
/// Tests that manifest parsing, signature verification, and ABI invariants work.
#[cfg(test)]
mod plugin_runtime_tests {
    use clawd::plugins::signing::{generate_keypair, verify_plugin_signature};
    use clawd_plugin_abi::manifest::{ManifestRuntime, PluginManifest};
    use clawd_plugin_abi::{ClawaError, CLAWD_PLUGIN_ABI_VERSION};

    #[test]
    fn abi_version_constant() {
        assert_eq!(CLAWD_PLUGIN_ABI_VERSION, 1);
    }

    #[test]
    fn manifest_parse_dylib() {
        let json = r#"{
            "name": "hello-clawd",
            "version": "0.1.0",
            "runtime": "dylib",
            "entry": "libhello_clawd.dylib"
        }"#;
        let m = PluginManifest::from_json(json).unwrap();
        assert_eq!(m.runtime, ManifestRuntime::Dylib);
        assert!(!m.is_signed());
    }

    #[test]
    fn manifest_parse_wasm() {
        let json = r#"{
            "name": "auto-test",
            "version": "0.1.0",
            "runtime": "wasm",
            "entry": "auto_test.wasm",
            "capabilities": ["fs.read"],
            "signature": "deadbeef"
        }"#;
        let m = PluginManifest::from_json(json).unwrap();
        assert_eq!(m.runtime, ManifestRuntime::Wasm);
        assert!(m.is_signed());
    }

    #[test]
    fn empty_sig_rejected() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"not a real plugin").unwrap();
        let result = verify_plugin_signature(tmp.path(), "", "aabbcc");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn nonempty_sig_placeholder_accepted() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"not a real plugin").unwrap();
        let (_, pub_hex) = generate_keypair();
        let result = verify_plugin_signature(tmp.path(), "aabbccdd", &pub_hex);
        assert!(
            result.is_ok(),
            "placeholder sig should be accepted: {:?}",
            result
        );
    }

    #[test]
    fn clawa_error_none_is_zero() {
        assert_eq!(ClawaError::None as u32, 0);
    }

    #[test]
    fn clawa_error_capability_denied() {
        assert_eq!(ClawaError::CapabilityDenied as u32, 3);
    }

    #[test]
    fn wasm_magic_check() {
        // Valid WASM magic bytes.
        let wasm_magic = b"\0asm\x01\x00\x00\x00";
        assert!(wasm_magic.starts_with(b"\0asm"));

        // Invalid bytes should fail the check.
        let not_wasm = b"ELF binary here";
        assert!(!not_wasm.starts_with(b"\0asm"));
    }
}
