// SPDX-License-Identifier: MIT
//! provider.rs — `daemon.checkProvider` RPC handler.
//!
//! Returns installation and authentication status for a given AI provider CLI.
//! This is the async, daemon-hosted counterpart to the synchronous checks in
//! `doctor.rs`. The daemon keeps a running server and exposes this via RPC so
//! that Flutter clients can display provider status without calling the CLI
//! themselves.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use tokio::process::Command;

/// `daemon.checkProvider` — check if a provider CLI is installed and authenticated.
///
/// Params:
/// ```json
/// { "provider": "claude" }   // or "codex"
/// ```
///
/// Returns:
/// ```json
/// {
///   "installed": true,
///   "authenticated": true,
///   "version": "1.2.3",
///   "path": "/usr/local/bin/claude"
/// }
/// ```
pub async fn check_provider(params: Value, _ctx: &AppContext) -> Result<Value> {
    let provider = params
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("claude")
        .to_string();

    match provider.as_str() {
        "claude" => check_claude_provider().await,
        "codex" => check_codex_provider().await,
        _ => Ok(json!({
            "installed": false,
            "authenticated": false,
            "version": null,
            "path": null,
            "error": format!("unknown provider: {provider}")
        })),
    }
}

// ─── Provider-specific checks ─────────────────────────────────────────────────

async fn check_claude_provider() -> Result<Value> {
    let (installed, version, path) = check_cli_installed("claude").await;

    if !installed {
        return Ok(json!({
            "installed": false,
            "authenticated": false,
            "version": null,
            "path": null,
        }));
    }

    let authenticated = check_claude_auth().await;

    Ok(json!({
        "installed": true,
        "authenticated": authenticated,
        "version": version,
        "path": path,
    }))
}

async fn check_codex_provider() -> Result<Value> {
    let (installed, version, path) = check_cli_installed("codex").await;

    if !installed {
        return Ok(json!({
            "installed": false,
            "authenticated": false,
            "version": null,
            "path": null,
        }));
    }

    // Codex authentication check: `codex --version` succeeding is sufficient
    // for now — codex uses API keys in environment variables rather than a
    // separate `auth status` command.
    Ok(json!({
        "installed": true,
        "authenticated": true,   // codex auth is env-var based, not CLI-managed
        "version": version,
        "path": path,
    }))
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

/// Run `{binary} --version` and return (installed, version_string, path).
/// Never fails — all errors are captured and returned as `installed: false`.
async fn check_cli_installed(binary: &str) -> (bool, Option<String>, Option<String>) {
    let result = Command::new(binary)
        .arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await;

    match result {
        Ok(out) if out.status.success() => {
            let version_raw = String::from_utf8_lossy(&out.stdout);
            let version = version_raw
                .lines()
                .next()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            // Resolve binary path via `which` equivalent — run `which {binary}`.
            let path = resolve_binary_path(binary).await;

            (true, version, path)
        }
        _ => (false, None, None),
    }
}

/// Resolve the full filesystem path of `binary` by running `which {binary}`.
/// Returns None if the binary is not found or `which` fails.
async fn resolve_binary_path(binary: &str) -> Option<String> {
    let which_cmd = if cfg!(windows) { "where" } else { "which" };
    let result = Command::new(which_cmd)
        .arg(binary)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;

    if result.status.success() {
        let path = String::from_utf8_lossy(&result.stdout)
            .lines()
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;
        Some(path)
    } else {
        None
    }
}

/// Run `claude auth status` and return true if the CLI reports it is logged in.
///
/// The claude CLI outputs something like "Logged in as user@example.com" when
/// authenticated. We treat any output containing "logged in" (case-insensitive)
/// that does not also contain "not logged in" as authenticated.
async fn check_claude_auth() -> bool {
    let result = Command::new("claude")
        .args(["auth", "status"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    match result {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();
            let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
            let combined = format!("{stdout}{stderr}");
            combined.contains("logged in") && !combined.contains("not logged in")
        }
        Err(_) => false,
    }
}
