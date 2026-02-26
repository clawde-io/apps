use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Cached result of the last `daemon.providers` check.
/// Avoids repeated subprocess calls on rapid polling.
static PROVIDERS_CACHE: OnceLock<Mutex<Option<(Instant, Value)>>> = OnceLock::new();
const PROVIDERS_CACHE_TTL: Duration = Duration::from_secs(30);

pub async fn ping(_params: Value, _ctx: &AppContext) -> Result<Value> {
    Ok(json!({ "pong": true }))
}

pub async fn status(_params: Value, ctx: &AppContext) -> Result<Value> {
    let uptime = ctx.started_at.elapsed().as_secs();
    let active_sessions = ctx.session_manager.active_count().await;
    let total_sessions = ctx.storage.count_sessions().await.unwrap_or(0);
    let watched_repos = ctx.repo_registry.watched_count().await;
    let pending_update = ctx.updater.pending_update().await.map(|u| u.version);

    // Build provider profiles array for the response
    let providers: Vec<Value> = ["claude", "codex", "cursor"]
        .iter()
        .map(|name| {
            let profile = ctx.config.provider_profile(name);
            json!({
                "name": name,
                "timeout": profile.and_then(|p| p.timeout),
                "maxTokens": profile.and_then(|p| p.max_tokens),
                "systemPromptPrefix": profile.and_then(|p| p.system_prompt_prefix.clone()),
            })
        })
        .collect();

    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "daemonId": ctx.daemon_id,
        "uptime": uptime,
        "activeSessions": active_sessions,
        "totalSessions": total_sessions,
        "watchedRepos": watched_repos,
        "port": ctx.config.port,
        "pendingUpdate": pending_update,
        "providers": providers,
        "recoveryMode": ctx.recovery_mode,
    }))
}

// ─── daemon.changelog (Sprint BB — UX.3) ─────────────────────────────────────

/// Embedded changelog JSON (compile-time include).
const CHANGELOG_JSON: &str = include_str!("../../changelog.json");

/// `daemon.changelog` — What's New overlay data (UX.3 — Sprint BB).
///
/// Returns new changelog entries since `last_seen_version` and marks the
/// current version as seen in the `settings` table.
///
/// Params: `{}` (none)
///
/// Response:
/// ```json
/// {
///   "currentVersion": "0.2.1",
///   "hasNew": true,
///   "entries": [
///     { "version": "0.2.1", "date": "…", "headline": "…", "entries": ["…"] }
///   ]
/// }
/// ```
pub async fn changelog(_params: Value, ctx: &AppContext) -> Result<Value> {
    let current_version = env!("CARGO_PKG_VERSION");

    // Read last seen version from settings table
    let last_seen = ctx
        .storage
        .get_setting("last_seen_version")
        .await
        .unwrap_or(None);

    // Parse the embedded changelog
    let all_entries: Vec<Value> =
        serde_json::from_str(CHANGELOG_JSON).unwrap_or_default();

    // Filter to entries newer than what was last seen
    let new_entries: Vec<Value> = if let Some(ref seen) = last_seen {
        all_entries
            .into_iter()
            .filter(|e| {
                e.get("version")
                    .and_then(|v| v.as_str())
                    .map(|v| semver_gt(v, seen.as_str()))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        // First launch — show the entry for the current version only
        all_entries
            .into_iter()
            .filter(|e| {
                e.get("version")
                    .and_then(|v| v.as_str())
                    .map(|v| v == current_version)
                    .unwrap_or(false)
            })
            .collect()
    };

    // Mark current version as seen (fire-and-forget; errors are non-fatal)
    let _ = ctx
        .storage
        .set_setting("last_seen_version", current_version)
        .await;

    Ok(json!({
        "currentVersion": current_version,
        "hasNew": !new_entries.is_empty(),
        "entries": new_entries,
    }))
}

/// Simple semver greater-than comparison (major.minor.patch only, no pre-release).
fn semver_gt(a: &str, b: &str) -> bool {
    fn parse(s: &str) -> (u32, u32, u32) {
        let mut parts = s.splitn(3, '.');
        let major = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    }
    parse(a) > parse(b)
}

pub async fn check_update(_params: Value, ctx: &AppContext) -> Result<Value> {
    let (current, latest, available) = ctx.updater.check().await?;
    Ok(json!({
        "current": current,
        "latest": latest,
        "available": available
    }))
}

/// Returns a list of available providers with version and health info.
///
/// Response shape: `[{ name, available, version?, accounts }]`
///
/// Results are cached for 30 s to avoid repeated subprocess calls.
pub async fn providers(_params: Value, ctx: &AppContext) -> Result<Value> {
    let cache = PROVIDERS_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().await;

    // Return cached result if still fresh.
    if let Some((ts, ref cached)) = *guard {
        if ts.elapsed() < PROVIDERS_CACHE_TTL {
            return Ok(cached.clone());
        }
    }

    // ── Claude ────────────────────────────────────────────────────────────────
    let claude_entry = {
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::process::Command::new("claude")
                .arg("--version")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output(),
        )
        .await;

        match result {
            Ok(Ok(out)) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                json!({ "name": "claude", "available": true, "version": version, "accounts": 0 })
            }
            _ => json!({ "name": "claude", "available": false, "accounts": 0 }),
        }
    };

    // ── Codex ─────────────────────────────────────────────────────────────────
    let codex_accounts = ctx
        .account_registry
        .count_available_accounts(Some("codex"))
        .await;
    let codex_available = codex_accounts > 0;
    let codex_entry = json!({
        "name": "codex",
        "available": codex_available,
        "accounts": codex_accounts
    });

    // ── Cursor ────────────────────────────────────────────────────────────────
    let cursor_entry = json!({ "name": "cursor", "available": false, "accounts": 0 });

    let result = json!([claude_entry, codex_entry, cursor_entry]);
    *guard = Some((Instant::now(), result.clone()));
    Ok(result)
}

pub async fn apply_update(_params: Value, ctx: &AppContext) -> Result<Value> {
    // Refuse to apply an update while sessions are active to avoid interrupting
    // in-flight AI turns.  The Flutter UI should check activeSessions first.
    let active = ctx.session_manager.active_count().await;
    if active > 0 {
        return Err(anyhow::anyhow!(
            "SESSION_BUSY: {} active session(s) — wait for them to finish before updating",
            active
        ));
    }
    let applied = ctx.updater.apply_if_ready().await?;
    Ok(json!({ "applied": applied }))
}

/// `daemon.updatePolicy` — return the current update policy string.
///
/// Returns: `{ "policy": "auto" | "manual" | "never" }`
pub async fn update_policy(_params: Value, ctx: &AppContext) -> Result<Value> {
    let policy = ctx.updater.get_policy().await;
    Ok(json!({ "policy": policy }))
}

/// `daemon.setUpdatePolicy` — change the update policy at runtime.
///
/// Params: `{ "policy": "auto" | "manual" | "never" }`
/// Returns: `{}`
///
/// The change is in-memory only and is reset on daemon restart.
/// To make it permanent, set `update_policy` in `config.toml`.
pub async fn set_update_policy(params: Value, ctx: &AppContext) -> Result<Value> {
    #[derive(serde::Deserialize)]
    struct Params {
        policy: String,
    }
    let p: Params = serde_json::from_value(params)?;
    ctx.updater.set_policy(&p.policy).await?;
    Ok(json!({}))
}
