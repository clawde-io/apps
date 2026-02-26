// SPDX-License-Identifier: MIT
//! auto-test — Example ClawDE WASM plugin.
//!
//! Runs a test command when a task is marked done. The test results are
//! sent as a push event to all connected clients.
//!
//! This is a **Rust→WASM** example. Build with:
//!   cargo build --target wasm32-unknown-unknown --release
//!
//! The WASM runtime (wasmtime) exposes these host functions:
//! - `clawd_log(level: i32, msg_ptr: i32, msg_len: i32)`
//! - `clawd_send_event(method_ptr: i32, method_len: i32, params_ptr: i32, params_len: i32)`
//!
//! WASM plugins use a simpler ABI than dylib plugins — the WASM runtime
//! calls exported functions by name rather than loading a vtable struct.

/// Called by the daemon after the WASM module is instantiated.
/// Return 0 on success, non-zero on failure.
#[no_mangle]
pub extern "C" fn clawd_plugin_init() -> i32 {
    // Nothing to initialize for this simple plugin.
    0
}

/// Called by the daemon when a task transitions to "done" status.
///
/// `session_id_ptr` / `session_id_len` — UTF-8 session ID in WASM linear memory.
/// `task_id_ptr` / `task_id_len` — UTF-8 task ID in WASM linear memory.
#[no_mangle]
pub extern "C" fn on_task_done(
    session_id_ptr: i32,
    session_id_len: i32,
    task_id_ptr: i32,
    task_id_len: i32,
) -> i32 {
    // Read IDs from WASM linear memory.
    let session_id = read_str(session_id_ptr, session_id_len);
    let task_id = read_str(task_id_ptr, task_id_len);

    // Log that we're triggering tests.
    log(2, &format!("auto-test: task done ({}), running tests...", task_id));

    // In a real plugin, call `clawd_send_event` to trigger `builder.status` or
    // invoke the test runner. For this example, emit a fake test result event.
    let params = format!(
        r#"{{"sessionId":"{session_id}","taskId":"{task_id}","result":"pass","tests":3}}"#,
    );
    send_event("plugin.testResult", &params);

    0
}

// ─── Host function bindings ──────────────────────────────────────────────────

extern "C" {
    /// Host function: write a log message.
    fn clawd_log(level: i32, msg_ptr: i32, msg_len: i32);
    /// Host function: send a push event to all connected clients.
    fn clawd_send_event(
        method_ptr: i32,
        method_len: i32,
        params_ptr: i32,
        params_len: i32,
    );
}

fn log(level: i32, msg: &str) {
    unsafe {
        clawd_log(level, msg.as_ptr() as i32, msg.len() as i32);
    }
}

fn send_event(method: &str, params: &str) {
    unsafe {
        clawd_send_event(
            method.as_ptr() as i32,
            method.len() as i32,
            params.as_ptr() as i32,
            params.len() as i32,
        );
    }
}

fn read_str(ptr: i32, len: i32) -> String {
    if ptr == 0 || len <= 0 {
        return String::new();
    }
    let slice =
        unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    String::from_utf8_lossy(slice).into_owned()
}
