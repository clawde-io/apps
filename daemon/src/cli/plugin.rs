// SPDX-License-Identifier: MIT
//! Sprint FF PL.9 + PL.10 — Plugin CLI commands.
//!
//! `clawd plugin scaffold --type dylib|wasm --name <name>`
//! `clawd plugin sign <binary> --key <key.hex>`
//! `clawd plugin verify <binary> --pubkey <pubkey.hex>`

use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::client::DaemonClient;
use crate::plugins::signing::{generate_keypair, sign_plugin, verify_plugin_signature};

// ─── scaffold ────────────────────────────────────────────────────────────────

/// `clawd plugin scaffold --type dylib --name hello-clawd`
///
/// Creates a starter plugin project in `./plugins/<name>/`.
pub async fn scaffold(plugin_type: &str, name: &str) -> Result<()> {
    let dir = Path::new("plugins").join(name);
    std::fs::create_dir_all(&dir)?;

    let manifest = serde_json::json!({
        "name": name,
        "version": "0.1.0",
        "description": format!("A ClawDE {} plugin", plugin_type),
        "author": "",
        "runtime": plugin_type,
        "entry": if plugin_type == "wasm" {
            format!("{}.wasm", name.replace('-', "_"))
        } else {
            format!("lib{}.dylib", name.replace('-', "_"))
        },
        "capabilities": [],
        "signature": ""
    });

    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(dir.join("clawd-plugin.json"), &manifest_json)?;

    if plugin_type == "wasm" {
        // Scaffold AssemblyScript / Rust WASM starter note.
        let readme = format!(
            "# {name} — ClawDE WASM Plugin\n\n\
            Implement `clawd_plugin_init` in your WASM module.\n\
            See: https://github.com/clawde-io/apps/wiki/Developing/PluginABI\n"
        );
        std::fs::write(dir.join("README.md"), readme)?;
    } else {
        // Scaffold Rust dylib starter.
        let cargo_toml = format!(
            "[package]\n\
            name = \"{name}\"\n\
            version = \"0.1.0\"\n\
            edition = \"2021\"\n\n\
            [lib]\n\
            crate-type = [\"cdylib\"]\n\n\
            [dependencies]\n\
            clawd_plugin_abi = \"1\"\n"
        );
        let lib_rs = r#"use clawd_plugin_abi::{ClawaContext, ClawaError, ClawaPlugin, CLAWD_PLUGIN_ABI_VERSION};

static mut PLUGIN: ClawaPlugin = ClawaPlugin {
    abi_version: CLAWD_PLUGIN_ABI_VERSION,
    name: c"my-plugin".as_ptr(),
    version: c"0.1.0".as_ptr(),
    on_load: Some(on_load),
    on_unload: None,
    on_session_start: None,
    on_session_end: None,
    on_tool_call: None,
    on_message: None,
    _reserved: [None; 8],
};

unsafe extern "C" fn on_load(_ctx: *mut ClawaContext) -> ClawaError {
    ClawaError::None
}

#[no_mangle]
pub unsafe extern "C" fn clawd_plugin_init() -> *mut ClawaPlugin {
    &raw mut PLUGIN
}
"#;
        let src_dir = dir.join("src");
        std::fs::create_dir_all(&src_dir)?;
        std::fs::write(dir.join("Cargo.toml"), cargo_toml)?;
        std::fs::write(src_dir.join("lib.rs"), lib_rs)?;
    }

    println!("Plugin scaffolded at: {}", dir.display());
    println!("  clawd-plugin.json  — manifest");
    if plugin_type == "dylib" {
        println!("  Cargo.toml         — Rust dylib crate");
        println!("  src/lib.rs         — plugin entry point");
        println!("\nBuild: cargo build --release --manifest-path {}/Cargo.toml", dir.display());
    } else {
        println!("  README.md          — WASM plugin guide");
    }
    Ok(())
}

// ─── sign ────────────────────────────────────────────────────────────────────

/// `clawd plugin sign <binary> --key <key-hex-file>`
pub async fn sign(binary_path: &str, key_file: &str) -> Result<()> {
    let key_hex = std::fs::read_to_string(key_file)?.trim().to_owned();
    let sig = sign_plugin(Path::new(binary_path), &key_hex)?;
    println!("{}", sig);
    Ok(())
}

// ─── verify ──────────────────────────────────────────────────────────────────

/// `clawd plugin verify <binary> --pubkey <pubkey-hex>`
pub async fn verify(binary_path: &str, pubkey_hex: &str) -> Result<()> {
    // For verification we need the signature from the manifest.
    // Expect the manifest to be in the same dir as the binary.
    let binary = Path::new(binary_path);
    let manifest_path = binary
        .parent()
        .unwrap_or(Path::new("."))
        .join("clawd-plugin.json");
    let manifest_json = std::fs::read_to_string(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_json)?;
    let sig = manifest["signature"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("manifest missing 'signature' field"))?;

    verify_plugin_signature(binary, sig, pubkey_hex)?;
    println!("✅ Signature valid");
    Ok(())
}

// ─── genkey ──────────────────────────────────────────────────────────────────

/// `clawd plugin genkey` — generate a new signing keypair.
pub async fn genkey() -> Result<()> {
    let (priv_hex, pub_hex) = generate_keypair();
    println!("Private key (keep secret):");
    println!("{}", priv_hex);
    println!("\nPublic key (share with registry):");
    println!("{}", pub_hex);
    Ok(())
}

// ─── list ────────────────────────────────────────────────────────────────────

/// `clawd plugin list` — list installed plugins via daemon RPC.
pub async fn list(client: &mut DaemonClient) -> Result<()> {
    let result = client.call_once("plugin.list", serde_json::json!({})).await?;
    let plugins = result["plugins"].as_array().cloned().unwrap_or_default();
    if plugins.is_empty() {
        println!("No plugins installed.");
    } else {
        for p in &plugins {
            let name = p["name"].as_str().unwrap_or("?");
            let ver = p["version"].as_str().unwrap_or("?");
            let status = p["status"].as_str().unwrap_or("?");
            let runtime = p["runtime"].as_str().unwrap_or("?");
            println!("  {name}@{ver}  [{runtime}]  {status}");
        }
    }
    Ok(())
}
