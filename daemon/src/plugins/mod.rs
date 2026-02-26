// SPDX-License-Identifier: MIT
//! Sprint FF — Plugin Architecture module.
//!
//! Sub-modules:
//! - `dylib_runtime` — native .dylib/.so loader
//! - `wasm_runtime` — WebAssembly loader (wasmtime)
//! - `manager` — plugin lifecycle and event dispatch
//! - `signing` — Ed25519 signature helpers

pub mod dylib_runtime;
pub mod manager;
pub mod signing;
pub mod wasm_runtime;
