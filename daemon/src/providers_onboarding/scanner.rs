// SPDX-License-Identifier: MIT
//! Provider scanner — detects installed AI provider CLIs and their auth state.
//!
//! Covers PO.T01: detect claude, codex, cursor — installation, authentication,
//! version string, and number of configured accounts.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, warn};

/// Per-provider status returned by `check_provider_status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    /// True if the CLI binary is found in PATH.
    pub installed: bool,
    /// True if the CLI reports an active authentication session.
    pub authenticated: bool,
    /// Version string from `{binary} --version` (None if not installed).
    pub version: Option<String>,
    /// Resolved filesystem path (None if not installed).
    pub path: Option<String>,
    /// Number of configured accounts detected (best-effort, may be 0).
    pub accounts_count: u32,
}

/// Capability summary for a single account returned by `account.capabilities`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountCapabilities {
    /// Provider name: "claude", "codex", or "cursor".
    pub provider: String,
    /// Subscription tier, e.g. "free", "pro", "max".
    pub tier: String,
    /// Known rate limits (requests per minute / tokens per minute), if available.
    pub rate_limits: Option<RateLimits>,
    /// ISO 8601 timestamp of the last successful request, if any.
    pub last_used: Option<String>,
    /// Fraction of requests that succeeded (0.0–1.0).
    pub success_rate: f32,
    /// ISO 8601 timestamp until which this account is cooling down, if any.
    pub cooldown_until: Option<String>,
}

/// Simple rate limit descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimits {
    /// Requests per minute allowed.
    pub rpm: Option<u32>,
    /// Tokens per minute allowed.
    pub tpm: Option<u32>,
}

/// Check the status of all three providers in parallel.
///
/// Returns a map of provider name → `ProviderStatus`.
pub async fn check_all_providers() -> Result<Vec<(String, ProviderStatus)>> {
    let (claude, codex, cursor) = tokio::join!(
        check_claude(),
        check_codex(),
        check_cursor(),
    );

    Ok(vec![
        ("claude".to_string(), claude?),
        ("codex".to_string(), codex?),
        ("cursor".to_string(), cursor?),
    ])
}

/// Check a single provider by name. Returns an error only on internal failure;
/// `installed: false` is the normal result when a CLI is absent.
pub async fn check_provider_by_name(provider: &str) -> Result<ProviderStatus> {
    match provider {
        "claude" => check_claude().await,
        "codex" => check_codex().await,
        "cursor" => check_cursor().await,
        other => {
            warn!(provider = other, "unknown provider requested in scanner");
            Ok(ProviderStatus {
                installed: false,
                authenticated: false,
                version: None,
                path: None,
                accounts_count: 0,
            })
        }
    }
}

// ─── Per-provider checks ──────────────────────────────────────────────────────

async fn check_claude() -> Result<ProviderStatus> {
    let (installed, version, path) = detect_binary("claude").await;
    if !installed {
        return Ok(ProviderStatus {
            installed: false,
            authenticated: false,
            version: None,
            path: None,
            accounts_count: 0,
        });
    }

    let authenticated = check_claude_auth().await;
    let accounts_count = count_claude_accounts().await;

    Ok(ProviderStatus {
        installed: true,
        authenticated,
        version,
        path,
        accounts_count,
    })
}

async fn check_codex() -> Result<ProviderStatus> {
    let (installed, version, path) = detect_binary("codex").await;
    if !installed {
        return Ok(ProviderStatus {
            installed: false,
            authenticated: false,
            version: None,
            path: None,
            accounts_count: 0,
        });
    }

    // Codex uses environment variable API keys, not a session login flow.
    // We consider it "authenticated" if the binary is installed and either
    // OPENAI_API_KEY or CODEX_API_KEY is set in the environment.
    let authenticated = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("CODEX_API_KEY"))
        .is_ok();

    Ok(ProviderStatus {
        installed: true,
        authenticated,
        version,
        path,
        accounts_count: if authenticated { 1 } else { 0 },
    })
}

async fn check_cursor() -> Result<ProviderStatus> {
    let (installed, version, path) = detect_binary("cursor").await;
    if !installed {
        return Ok(ProviderStatus {
            installed: false,
            authenticated: false,
            version: None,
            path: None,
            accounts_count: 0,
        });
    }

    let (authenticated, accounts_count) = check_cursor_auth().await;

    Ok(ProviderStatus {
        installed: true,
        authenticated,
        version,
        path,
        accounts_count,
    })
}

// ─── Auth detectors ───────────────────────────────────────────────────────────

/// Run `claude auth status` and parse the output.
async fn check_claude_auth() -> bool {
    let result = Command::new("claude")
        .args(["auth", "status"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    match result {
        Ok(out) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout).to_lowercase(),
                String::from_utf8_lossy(&out.stderr).to_lowercase()
            );
            combined.contains("logged in") && !combined.contains("not logged in")
        }
        Err(e) => {
            debug!(err = %e, "claude auth status failed");
            false
        }
    }
}

/// Count Claude accounts by inspecting `~/.claude/` account directory entries.
///
/// Claude Code stores one file per account under `~/.claude/`. We count
/// `*.json` files that look like account credentials. Returns 0 on any error.
async fn count_claude_accounts() -> u32 {
    let home = match home_dir() {
        Some(h) => h,
        None => return 0,
    };

    // Claude Code stores credentials in ~/.claude/
    // Try to count credential-like JSON files.
    let claude_dir = home.join(".claude");
    if !claude_dir.exists() {
        return 0;
    }

    // The presence of ~/.claude/credentials.json or similar files signals 1 account.
    // More sophisticated discovery would parse the config, but for onboarding a
    // binary "has account / does not" is sufficient.
    let cred_file = claude_dir.join("credentials.json");
    let settings_file = claude_dir.join("settings.json");

    if cred_file.exists() || settings_file.exists() {
        1
    } else {
        0
    }
}

/// Read `~/.cursor/auth.json` to determine Cursor auth state and account count.
async fn check_cursor_auth() -> (bool, u32) {
    let home = match home_dir() {
        Some(h) => h,
        None => return (false, 0),
    };

    let auth_path = home.join(".cursor").join("auth.json");
    if !auth_path.exists() {
        return (false, 0);
    }

    // Read and parse auth.json — look for a non-empty access token.
    let content = match tokio::fs::read_to_string(&auth_path).await {
        Ok(c) => c,
        Err(e) => {
            debug!(err = %e, "failed to read cursor auth.json");
            return (false, 0);
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            debug!(err = %e, "failed to parse cursor auth.json");
            return (false, 0);
        }
    };

    // auth.json typically contains { "accessToken": "...", "refreshToken": "..." }
    let has_token = json
        .get("accessToken")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    if has_token { (true, 1) } else { (false, 0) }
}

// ─── Binary detection helpers ─────────────────────────────────────────────────

/// Run `{binary} --version` and return (installed, version, path).
/// Uses the system PATH; never panics.
async fn detect_binary(binary: &str) -> (bool, Option<String>, Option<String>) {
    let result = Command::new(binary)
        .arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await;

    match result {
        Ok(out) if out.status.success() => {
            let raw = String::from_utf8_lossy(&out.stdout);
            let version = raw
                .lines()
                .next()
                .map(|l| l.trim().to_string())
                .filter(|s| !s.is_empty());

            let path = resolve_path(binary).await;
            (true, version, path)
        }
        _ => (false, None, None),
    }
}

/// Resolve the absolute path of a binary via `which` / `where`.
async fn resolve_path(binary: &str) -> Option<String> {
    let which_cmd = if cfg!(windows) { "where" } else { "which" };
    let out = Command::new(which_cmd)
        .arg(binary)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;

    if out.status.success() {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .map(|l| l.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}

/// Return the current user's home directory.
fn home_dir() -> Option<std::path::PathBuf> {
    #[allow(deprecated)] // std::env::home_dir deprecated, but we use it safely
    std::env::home_dir()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_status_not_installed_defaults() {
        let ps = ProviderStatus {
            installed: false,
            authenticated: false,
            version: None,
            path: None,
            accounts_count: 0,
        };
        assert!(!ps.installed);
        assert!(!ps.authenticated);
        assert!(ps.version.is_none());
        assert!(ps.path.is_none());
        assert_eq!(ps.accounts_count, 0);
    }

    #[test]
    fn provider_status_installed_fields() {
        let ps = ProviderStatus {
            installed: true,
            authenticated: true,
            version: Some("1.2.3".to_string()),
            path: Some("/usr/local/bin/claude".to_string()),
            accounts_count: 2,
        };
        assert!(ps.installed);
        assert_eq!(ps.version.as_deref(), Some("1.2.3"));
        assert_eq!(ps.accounts_count, 2);
    }

    #[test]
    fn provider_status_roundtrip_json() {
        let ps = ProviderStatus {
            installed: true,
            authenticated: false,
            version: Some("0.9.0".to_string()),
            path: None,
            accounts_count: 0,
        };
        let json = serde_json::to_string(&ps).unwrap();
        let back: ProviderStatus = serde_json::from_str(&json).unwrap();
        assert!(back.installed);
        assert!(!back.authenticated);
        assert_eq!(back.version.as_deref(), Some("0.9.0"));
    }

    #[test]
    fn account_capabilities_optional_rate_limits() {
        let ac = AccountCapabilities {
            provider: "claude".to_string(),
            tier: "pro".to_string(),
            rate_limits: Some(RateLimits { rpm: Some(60), tpm: Some(100_000) }),
            last_used: None,
            success_rate: 0.99,
            cooldown_until: None,
        };
        assert_eq!(ac.provider, "claude");
        assert!(ac.rate_limits.is_some());
        let rl = ac.rate_limits.unwrap();
        assert_eq!(rl.rpm, Some(60));
    }

    #[test]
    fn rate_limits_both_none() {
        let rl = RateLimits { rpm: None, tpm: None };
        assert!(rl.rpm.is_none());
        assert!(rl.tpm.is_none());
    }

    #[tokio::test]
    async fn check_provider_by_name_unknown_returns_not_installed() {
        // "definitely-not-a-real-provider" is not installed on any CI machine
        let result = check_provider_by_name("definitely-not-a-real-provider").await;
        let ps = result.expect("should not error — returns not-installed for unknown");
        assert!(!ps.installed);
        assert!(!ps.authenticated);
        assert_eq!(ps.accounts_count, 0);
    }
}
