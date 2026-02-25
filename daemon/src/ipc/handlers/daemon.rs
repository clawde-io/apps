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
        "providers": providers
    }))
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
