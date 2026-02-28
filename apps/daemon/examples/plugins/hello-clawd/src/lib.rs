// SPDX-License-Identifier: MIT
//! hello-clawd — Example ClawDE dylib plugin.
//!
//! Demonstrates the minimal structure required for a native plugin.
//! On session start it logs "Hello from hello-clawd!" to the daemon log.
//!
//! Build:
//!   cargo build --release --manifest-path examples/plugins/hello-clawd/Cargo.toml
//!
//! The output `libhello_clawd.dylib` (macOS) / `libhello_clawd.so` (Linux)
//! can then be installed as a ClawDE plugin pack.

use std::ffi::CStr;

use clawd_plugin_abi::{
    ClawaContext, ClawaError, ClawaPlugin, CLAWD_PLUGIN_ABI_VERSION,
};

// ─── Static vtable ────────────────────────────────────────────────────────────

/// Plugin vtable — static, lives for the lifetime of the loaded library.
///
/// SAFETY: `static mut` is used here because the C ABI requires a stable
/// `*mut ClawaPlugin`. The daemon guarantees single-threaded access to the
/// vtable (one init call, then immutable use).
static mut PLUGIN_VTABLE: ClawaPlugin = ClawaPlugin {
    abi_version: CLAWD_PLUGIN_ABI_VERSION,
    name: c"hello-clawd".as_ptr(),
    version: c"0.1.0".as_ptr(),
    on_load: Some(on_load),
    on_unload: Some(on_unload),
    on_session_start: Some(on_session_start),
    on_session_end: None,
    on_tool_call: None,
    on_message: None,
    _reserved: [None; 8],
};

// ─── Required export ──────────────────────────────────────────────────────────

/// Entry point called by the ClawDE daemon after dlopen.
///
/// Returns a pointer to the plugin vtable. The daemon owns the pointer
/// for the plugin's lifetime — do not free it.
#[no_mangle]
pub unsafe extern "C" fn clawd_plugin_init() -> *mut ClawaPlugin {
    #[allow(static_mut_refs)]
    &raw mut PLUGIN_VTABLE
}

// ─── Callbacks ────────────────────────────────────────────────────────────────

unsafe extern "C" fn on_load(ctx: *mut ClawaContext) -> ClawaError {
    log_message(ctx, 2, "hello-clawd loaded! Ready to greet sessions.");
    ClawaError::None
}

unsafe extern "C" fn on_unload(ctx: *mut ClawaContext) {
    log_message(ctx, 2, "hello-clawd unloading. Goodbye!");
}

unsafe extern "C" fn on_session_start(
    ctx: *mut ClawaContext,
    session_id: *const core::ffi::c_char,
) -> ClawaError {
    let id = if session_id.is_null() {
        "unknown".to_owned()
    } else {
        CStr::from_ptr(session_id).to_string_lossy().into_owned()
    };
    let msg = format!("Hello from hello-clawd! Session started: {}", id);
    log_message(ctx, 2, &msg);
    ClawaError::None
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Write a log message via the daemon context's `log` function pointer.
///
/// Level: 0=trace 1=debug 2=info 3=warn 4=error
unsafe fn log_message(ctx: *mut ClawaContext, level: u8, msg: &str) {
    if ctx.is_null() {
        return;
    }
    let ctx_ref = &*ctx;
    // Build a C string. On failure (embedded null), truncate to the safe prefix.
    if let Ok(cstr) = std::ffi::CString::new(msg) {
        (ctx_ref.log)(ctx, level, cstr.as_ptr());
    }
}
