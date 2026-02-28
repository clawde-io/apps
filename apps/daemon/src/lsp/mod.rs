// SPDX-License-Identifier: MIT
pub mod handlers;
/// LSP (Language Server Protocol) proxy — Sprint S, LS.T01–LS.T04.
///
/// Manages per-language LSP server processes, proxies LSP requests from the
/// daemon's JSON-RPC layer, and exposes diagnostics and completions to Flutter
/// clients via the `lsp.*` RPC family.
///
/// ## Module layout
///
/// - `model`    — data types (LspConfig, LspProcess, DiagnosticItem, CompletionItem)
/// - `proxy`    — LspProxy: subprocess lifecycle + JSON-RPC-over-stdio transport
/// - `handlers` — RPC handler functions wired into the dispatch table
pub mod model;
pub mod proxy;

pub use model::{CompletionItem, DiagSeverity, DiagnosticItem, LspConfig, LspProcess};
pub use proxy::LspProxy;
