// SPDX-License-Identifier: MIT
//! # `clawd_plugin_abi` — Stable C ABI for ClawDE Plugins
//!
//! This crate defines the **stable** C ABI that all ClawDE plugins must implement.
//! The ABI is declared STABLE at v1.0.0 — no breaking changes will be made
//! without a major version bump and a compatibility shim.
//!
//! ## Plugin types
//!
//! | Runtime | Extension | Entry point |
//! |---------|-----------|-------------|
//! | Dynamic library | `.dylib` / `.so` / `.dll` | `clawd_plugin_init()` C export |
//! | WebAssembly | `.wasm` | `clawd_plugin_init` wasm export |
//!
//! ## ABI stability guarantee (v1.0.0)
//!
//! - All structs in this crate are `#[repr(C)]`.
//! - Function pointer signatures will not change in minor releases.
//! - New optional callbacks may be added via reserved fields.
//! - The `abi_version` field lets the daemon reject incompatible plugins.

pub mod manifest;

/// ABI version baked into this crate. Plugins built against a different
/// major version will be rejected at load time.
pub const CLAWD_PLUGIN_ABI_VERSION: u32 = 1;

// ─── Error type ──────────────────────────────────────────────────────────────

/// Errors a plugin can return to the daemon.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClawaError {
    /// No error.
    None = 0,
    /// Plugin initialization failed.
    InitFailed = 1,
    /// Callback returned an unexpected error.
    CallbackError = 2,
    /// Plugin attempted a disallowed operation (capability violation).
    CapabilityDenied = 3,
    /// Plugin requested a resource that does not exist.
    NotFound = 4,
    /// Internal daemon error.
    DaemonError = 5,
}

// ─── Event types ─────────────────────────────────────────────────────────────

/// Events the daemon delivers to plugins.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClawaEventType {
    /// A new session was created.
    SessionStart = 0,
    /// A session ended (completed or cancelled).
    SessionEnd = 1,
    /// A tool was called by the AI.
    ToolCall = 2,
    /// A message was created in a session.
    Message = 3,
    /// A task changed status.
    TaskStatus = 4,
    /// A git commit was detected.
    GitCommit = 5,
    /// Daemon is shutting down cleanly.
    Shutdown = 6,
}

// ─── Context handle ──────────────────────────────────────────────────────────

/// Opaque handle the daemon passes to every plugin callback.
///
/// Use the function pointers in this struct to call back into the daemon.
/// Do NOT store this pointer — it is only valid during the callback.
#[repr(C)]
pub struct ClawaContext {
    /// Pointer to internal daemon state. Treat as opaque.
    pub _inner: *mut core::ffi::c_void,

    /// Send a JSON push event to all connected clients.
    ///
    /// `method` — null-terminated UTF-8 event name (e.g. `"plugin.myEvent\0"`)
    /// `params_json` — null-terminated UTF-8 JSON object string
    ///
    /// Returns `ClawaError::None` on success.
    pub send_event: unsafe extern "C" fn(
        ctx: *mut ClawaContext,
        method: *const core::ffi::c_char,
        params_json: *const core::ffi::c_char,
    ) -> ClawaError,

    /// Write a log message to the daemon's structured log.
    ///
    /// `level` — 0=trace 1=debug 2=info 3=warn 4=error
    /// `msg` — null-terminated UTF-8 string
    pub log: unsafe extern "C" fn(
        ctx: *mut ClawaContext,
        level: u8,
        msg: *const core::ffi::c_char,
    ),

    /// Reserved for future host functions. Must be set to null.
    pub _reserved: [*mut core::ffi::c_void; 8],
}

// ─── Plugin vtable ───────────────────────────────────────────────────────────

/// Plugin vtable — the stable interface every plugin must implement.
///
/// The daemon calls `clawd_plugin_init()` once at load time. The plugin
/// fills in this struct and returns it to the daemon. The daemon then
/// calls the appropriate function pointers on each event.
///
/// All function pointers are nullable — set to null to opt out of an event.
#[repr(C)]
pub struct ClawaPlugin {
    /// Must be `CLAWD_PLUGIN_ABI_VERSION`. The daemon will reject mismatches.
    pub abi_version: u32,

    /// Null-terminated UTF-8 plugin name (e.g. `"hello-clawd\0"`).
    pub name: *const core::ffi::c_char,

    /// Null-terminated semver string (e.g. `"1.0.0\0"`).
    pub version: *const core::ffi::c_char,

    /// Called once immediately after `clawd_plugin_init()` returns.
    /// Use for one-time setup. Return `ClawaError::None` on success.
    pub on_load: Option<unsafe extern "C" fn(ctx: *mut ClawaContext) -> ClawaError>,

    /// Called before daemon shutdown. Release resources here.
    pub on_unload: Option<unsafe extern "C" fn(ctx: *mut ClawaContext)>,

    /// Called when a session lifecycle event fires.
    /// `event_json` — null-terminated UTF-8 JSON payload for the event.
    pub on_session_start: Option<
        unsafe extern "C" fn(
            ctx: *mut ClawaContext,
            session_id: *const core::ffi::c_char,
        ) -> ClawaError,
    >,

    /// Called when a session ends (status → done/error/cancelled).
    pub on_session_end: Option<
        unsafe extern "C" fn(
            ctx: *mut ClawaContext,
            session_id: *const core::ffi::c_char,
        ) -> ClawaError,
    >,

    /// Called when the AI invokes a tool.
    /// `tool_name` — null-terminated tool name.
    /// `input_json` — null-terminated JSON input object.
    pub on_tool_call: Option<
        unsafe extern "C" fn(
            ctx: *mut ClawaContext,
            session_id: *const core::ffi::c_char,
            tool_name: *const core::ffi::c_char,
            input_json: *const core::ffi::c_char,
        ) -> ClawaError,
    >,

    /// Called when a message is created in a session.
    /// `role` — null-terminated: `"user"` or `"assistant"`.
    /// `content_json` — null-terminated JSON string content.
    pub on_message: Option<
        unsafe extern "C" fn(
            ctx: *mut ClawaContext,
            session_id: *const core::ffi::c_char,
            role: *const core::ffi::c_char,
            content_json: *const core::ffi::c_char,
        ) -> ClawaError,
    >,

    /// Reserved for future callbacks. Must be set to null.
    pub _reserved: [Option<unsafe extern "C" fn()>; 8],
}

// Safety: ClawaPlugin contains raw pointers but we document that the daemon
// is the sole owner of the context pointer and plugin lifetime.
unsafe impl Send for ClawaPlugin {}
unsafe impl Sync for ClawaPlugin {}

// ─── Required export ─────────────────────────────────────────────────────────

/// Type alias for the required export that every plugin must provide.
///
/// Plugins must export a C function named `clawd_plugin_init` with this
/// signature. The daemon will dlopen the plugin and call this function.
///
/// ```c
/// // C declaration:
/// ClawaPlugin* clawd_plugin_init(void);
/// ```
pub type ClawaPluginInitFn = unsafe extern "C" fn() -> *mut ClawaPlugin;

/// Name of the required plugin export.
pub const CLAWD_PLUGIN_INIT_SYMBOL: &[u8] = b"clawd_plugin_init\0";

// ─── Plugin manifest types ───────────────────────────────────────────────────

/// Plugin runtime type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClawaRuntime {
    /// Native dynamic library (`.dylib` / `.so` / `.dll`).
    Dylib,
    /// WebAssembly module (`.wasm`).
    Wasm,
}

/// Capability a plugin may request. The daemon enforces these at call time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClawaCapability {
    /// Read files in the project directory.
    FsRead,
    /// Write files in the project directory.
    FsWrite,
    /// Send events to connected clients.
    Network,
    /// Call daemon RPC methods.
    Rpc,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_one() {
        assert_eq!(CLAWD_PLUGIN_ABI_VERSION, 1);
    }

    #[test]
    fn init_symbol_is_correct() {
        let s = core::str::from_utf8(CLAWD_PLUGIN_INIT_SYMBOL).unwrap();
        assert_eq!(s, "clawd_plugin_init\0");
    }

    #[test]
    fn clawa_error_repr_c() {
        assert_eq!(ClawaError::None as u32, 0);
        assert_eq!(ClawaError::CapabilityDenied as u32, 3);
    }

    #[test]
    fn event_type_variants() {
        assert_eq!(ClawaEventType::SessionStart as u32, 0);
        assert_eq!(ClawaEventType::Shutdown as u32, 6);
    }
}
