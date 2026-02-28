# Plugin ABI Reference

> `clawd_plugin_abi` v1.0.0 — **STABLE**. No breaking changes without a major version bump.

## Overview

The `clawd_plugin_abi` crate defines the stable C ABI that all ClawDE daemon plugins must implement. It is published to crates.io at `v1.0.0`.

Add to your plugin's `Cargo.toml`:

```toml
[dependencies]
clawd_plugin_abi = "1"
```

## Required Export

Every plugin (dylib or WASM) must export `clawd_plugin_init`:

```rust
#[no_mangle]
pub unsafe extern "C" fn clawd_plugin_init() -> *mut ClawaPlugin {
    // Return a pointer to your plugin vtable.
    &raw mut MY_PLUGIN
}
```

The daemon calls this once after `dlopen`. The pointer must remain valid for the plugin's lifetime.

## ClawaPlugin vtable

```rust
#[repr(C)]
pub struct ClawaPlugin {
    pub abi_version: u32,                     // Must be CLAWD_PLUGIN_ABI_VERSION
    pub name: *const c_char,                  // Null-terminated plugin name
    pub version: *const c_char,               // Null-terminated semver
    pub on_load: Option<OnLoadFn>,            // Called after init — setup here
    pub on_unload: Option<OnUnloadFn>,        // Called before unload — cleanup here
    pub on_session_start: Option<...>,        // Session lifecycle
    pub on_session_end: Option<...>,          // Session lifecycle
    pub on_tool_call: Option<...>,            // AI tool calls
    pub on_message: Option<...>,              // Session messages
    _reserved: [Option<fn()>; 8],             // Must be [None; 8]
}
```

Set any callback to `None` to opt out of that event. The daemon checks for null before calling.

## ClawaContext

Passed as first argument to every callback. Do NOT store — valid only during the callback.

```rust
#[repr(C)]
pub struct ClawaContext {
    pub _inner: *mut c_void,                  // Opaque — do not dereference
    pub send_event: unsafe extern "C" fn(...) -> ClawaError,
    pub log: unsafe extern "C" fn(ctx, level: u8, msg: *const c_char),
    _reserved: [*mut c_void; 8],
}
```

### Logging

```rust
// Level: 0=trace 1=debug 2=info 3=warn 4=error
(ctx.log)(ctx, 2, c"my plugin message".as_ptr());
```

### Sending events

```rust
(ctx.send_event)(
    ctx,
    c"plugin.myEvent".as_ptr(),
    c"{\"key\":\"value\"}".as_ptr(),
);
```

## ClawaError

```rust
pub enum ClawaError {
    None = 0,
    InitFailed = 1,
    CallbackError = 2,
    CapabilityDenied = 3,
    NotFound = 4,
    DaemonError = 5,
}
```

Return `ClawaError::None` from all callbacks on success.

## ABI Version

```rust
pub const CLAWD_PLUGIN_ABI_VERSION: u32 = 1;
```

The daemon rejects any plugin whose `abi_version` field differs from the daemon's compiled constant. Plugins must use the same major version.

## Stability Guarantee

| What is stable | What may change |
| --- | --- |
| All `#[repr(C)]` struct layouts | Internal daemon state (`_inner`) |
| All callback function signatures | Reserved fields (will add new callbacks via reserved slots) |
| `ClawaError` discriminants 0–5 | Error codes beyond 5 (may add new variants) |
| `CLAWD_PLUGIN_ABI_VERSION = 1` for v1.x | Will bump to 2 for any breaking change |

## Full Example

See `examples/plugins/hello-clawd/` in the `apps` repository for a complete dylib plugin implementation.
