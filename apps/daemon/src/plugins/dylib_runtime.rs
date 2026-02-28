// SPDX-License-Identifier: MIT
//! Sprint FF PL.2 — Native dynamic library plugin runtime.
//!
//! Loads `.dylib` / `.so` / `.dll` plugins via `libloading`.
//! Verifies Ed25519 signature on the binary before loading.
//! (Sandbox via seccomp/sandbox profile is a deployment-time concern.)

use std::path::Path;

use anyhow::{bail, Context, Result};
use libloading::{Library, Symbol};

use clawd_plugin_abi::{
    ClawaContext, ClawaError, ClawaPlugin, CLAWD_PLUGIN_ABI_VERSION, CLAWD_PLUGIN_INIT_SYMBOL,
};

use super::signing::verify_plugin_signature;

/// A loaded dylib plugin instance.
pub struct DylibPlugin {
    /// The underlying loaded library. Must outlive the plugin vtable pointer.
    _lib: Library,
    /// Pointer to the plugin vtable returned by `clawd_plugin_init()`.
    plugin: *mut ClawaPlugin,
    /// Display name (from manifest, set after load).
    pub name: String,
}

// SAFETY: DylibPlugin is accessed only from the plugin manager's dedicated
// tokio blocking thread. The raw pointer is valid for the lifetime of `_lib`.
unsafe impl Send for DylibPlugin {}
unsafe impl Sync for DylibPlugin {}

impl DylibPlugin {
    /// Load a plugin from a `.dylib`/`.so`/`.dll` binary.
    ///
    /// Steps:
    /// 1. Verify Ed25519 signature (if `expected_sig` is `Some`).
    /// 2. `dlopen` the binary.
    /// 3. Resolve `clawd_plugin_init` symbol.
    /// 4. Call the init function, get plugin vtable.
    /// 5. Verify ABI version.
    pub fn load(
        binary_path: &Path,
        expected_sig: Option<&str>,
        signer_pubkey: Option<&str>,
    ) -> Result<Self> {
        // 1. Signature verification.
        if let (Some(sig), Some(pubkey)) = (expected_sig, signer_pubkey) {
            verify_plugin_signature(binary_path, sig, pubkey)
                .context("plugin signature verification failed")?;
        }

        // 2. dlopen.
        // SAFETY: We've verified the binary above. Library loading is inherently
        // unsafe — we document that only verified plugins should be loaded.
        let lib = unsafe {
            Library::new(binary_path)
                .with_context(|| format!("failed to open plugin: {}", binary_path.display()))?
        };

        // 3. Resolve init symbol.
        let init_fn: Symbol<unsafe extern "C" fn() -> *mut ClawaPlugin> = unsafe {
            lib.get(CLAWD_PLUGIN_INIT_SYMBOL)
                .context("plugin missing required `clawd_plugin_init` export")?
        };

        // 4. Call init.
        let plugin = unsafe { init_fn() };
        if plugin.is_null() {
            bail!("clawd_plugin_init() returned null");
        }

        // 5. ABI version check.
        let plugin_ref = unsafe { &*plugin };
        if plugin_ref.abi_version != CLAWD_PLUGIN_ABI_VERSION {
            bail!(
                "plugin ABI version mismatch: expected {}, got {}",
                CLAWD_PLUGIN_ABI_VERSION,
                plugin_ref.abi_version
            );
        }

        // Read name from plugin vtable.
        let name = if plugin_ref.name.is_null() {
            binary_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into())
        } else {
            unsafe {
                std::ffi::CStr::from_ptr(plugin_ref.name)
                    .to_string_lossy()
                    .into_owned()
            }
        };

        Ok(Self {
            _lib: lib,
            plugin,
            name,
        })
    }

    /// Call `on_load` if the plugin provides it.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn call_on_load(&self, ctx: *mut ClawaContext) -> ClawaError {
        let plugin = unsafe { &*self.plugin };
        if let Some(on_load) = plugin.on_load {
            unsafe { on_load(ctx) }
        } else {
            ClawaError::None
        }
    }

    /// Call `on_session_start` with the given session ID.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn call_on_session_start(
        &self,
        ctx: *mut ClawaContext,
        session_id: *const core::ffi::c_char,
    ) -> ClawaError {
        let plugin = unsafe { &*self.plugin };
        if let Some(f) = plugin.on_session_start {
            unsafe { f(ctx, session_id) }
        } else {
            ClawaError::None
        }
    }

    /// Call `on_unload` if the plugin provides it.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn call_on_unload(&self, ctx: *mut ClawaContext) {
        let plugin = unsafe { &*self.plugin };
        if let Some(on_unload) = plugin.on_unload {
            unsafe { on_unload(ctx) }
        }
    }
}
