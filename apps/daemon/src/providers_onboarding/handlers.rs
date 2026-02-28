// SPDX-License-Identifier: MIT
//! JSON-RPC handler functions for provider onboarding RPCs (Sprint I).
//!
//! Wired into `ipc/mod.rs` dispatch table by the wiring notes.
//!
//! RPCs implemented here:
//! - `provider.checkAll`     — status of all three providers (PO.T01)
//! - `account.addApiKey`     — validate and store an API key (PO.T03)
//! - `account.capabilities`  — per-account capability matrix (PO.T05)
//! - `gci.generate`          — generate CLAUDE.md from questionnaire (PO.T08)
//! - `gci.generateCodex`     — generate Codex AGENTS.md (PO.T09)
//! - `gci.generateCursor`    — generate Cursor rules file (PO.T10)
//! - `repo.bootstrapAid`     — bootstrap .claude/ AID for a repo (PO.T14–T15)
//! - `repo.checkAid`         — check if repo has AID (PO.T13)

use crate::AppContext;
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use super::{aid_bootstrapper, gci_generator, scanner};

// ─── provider.checkAll ────────────────────────────────────────────────────────

/// `provider.checkAll` — check all three providers in parallel.
///
/// Params: none required.
///
/// Returns:
/// ```json
/// {
///   "providers": {
///     "claude":  { "installed": true, "authenticated": true, "version": "1.x", "path": "...", "accountsCount": 1 },
///     "codex":   { "installed": false, ... },
///     "cursor":  { "installed": true, ... }
///   }
/// }
/// ```
pub async fn check_all(_params: Value, _ctx: &AppContext) -> Result<Value> {
    let statuses = scanner::check_all_providers().await?;

    let mut providers = serde_json::Map::new();
    for (name, status) in statuses {
        providers.insert(name, serde_json::to_value(&status)?);
    }

    Ok(json!({ "providers": providers }))
}

/// Extended `daemon.checkProvider` — single provider check.
///
/// Params: `{ "provider": "claude" | "codex" | "cursor" }`
pub async fn check_provider(params: Value, _ctx: &AppContext) -> Result<Value> {
    let provider = params
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("claude");

    let status = scanner::check_provider_by_name(provider).await?;
    Ok(serde_json::to_value(&status)?)
}

// ─── account.addApiKey ────────────────────────────────────────────────────────

/// `account.addApiKey` — validate an API key against the provider, then store it.
///
/// Params:
/// ```json
/// { "provider": "codex", "apiKey": "sk-...", "label": "Work OpenAI" }
/// ```
///
/// Validates the key with a lightweight HTTP request then writes an account row.
pub async fn add_api_key(params: Value, ctx: &AppContext) -> Result<Value> {
    let provider = params
        .get("provider")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: provider"))?
        .to_string();

    let api_key = params
        .get("apiKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: apiKey"))?
        .to_string();

    let label = params
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or(&provider)
        .to_string();

    // Validate the key online before persisting.
    validate_api_key(&provider, &api_key).await?;

    // Derive a credentials path: store the key as a JSON file under the daemon
    // data directory. The path is what the AccountRow tracks — the key itself
    // is written to a file so it is not inline in the DB.
    let data_dir = &ctx.config.data_dir;
    let key_file = data_dir
        .join("credentials")
        .join(format!("{provider}_{}.json", uuid::Uuid::new_v4()));

    // Create credentials directory.
    if let Some(parent) = key_file.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("failed to create credentials directory")?;
    }

    // Write key file as JSON.
    let cred_json = serde_json::json!({
        "provider": provider,
        "apiKey": api_key,
        "label": label,
        "addedAt": Utc::now().to_rfc3339(),
    });
    tokio::fs::write(&key_file, cred_json.to_string())
        .await
        .context("failed to write credential file")?;

    // Count existing accounts for this provider to set priority.
    let existing = ctx.storage.list_accounts().await?;
    let next_priority = existing.iter().filter(|a| a.provider == provider).count() as i64;

    // Register the account row in the DB.
    let account = ctx
        .storage
        .create_account(
            &label,
            &provider,
            &key_file.to_string_lossy(),
            next_priority,
        )
        .await
        .context("failed to store account row")?;

    info!(
        account_id = %account.id,
        provider,
        label,
        "API key account registered"
    );

    Ok(json!({
        "accountId": account.id,
        "provider": provider,
        "label": label,
        "addedAt": Utc::now().to_rfc3339(),
    }))
}

/// Validate an API key by calling the provider's models endpoint.
/// Returns `Ok(())` if the key is valid, `Err` otherwise.
async fn validate_api_key(provider: &str, api_key: &str) -> Result<()> {
    let (url, header_name) = match provider {
        "codex" | "openai" => ("https://api.openai.com/v1/models", "Authorization"),
        "claude" | "anthropic" => ("https://api.anthropic.com/v1/models", "x-api-key"),
        other => {
            warn!(
                provider = other,
                "unknown provider — skipping API key validation"
            );
            return Ok(());
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("failed to build HTTP client")?;

    let header_value = if header_name == "Authorization" {
        format!("Bearer {api_key}")
    } else {
        api_key.to_string()
    };

    let resp = client
        .get(url)
        .header(header_name, &header_value)
        .send()
        .await
        .context("API key validation request failed")?;

    if resp.status().is_success() {
        debug!(provider, "API key validated successfully");
        Ok(())
    } else if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        anyhow::bail!("API key rejected by {provider} — invalid or expired key")
    } else {
        // 429, 5xx — accept the key but warn; might be a transient error.
        warn!(
            provider,
            status = resp.status().as_u16(),
            "API key validation returned non-200; accepting key anyway"
        );
        Ok(())
    }
}

// ─── account.capabilities ─────────────────────────────────────────────────────

/// `account.capabilities` — return per-account capability matrix.
///
/// Params: `{ "accountId": "..." }` (optional) or `{}` for all accounts.
pub async fn account_capabilities(params: Value, ctx: &AppContext) -> Result<Value> {
    let accounts = ctx.storage.list_accounts().await?;
    let now = Utc::now();

    let requested_id = params
        .get("accountId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let filtered: Vec<_> = accounts
        .into_iter()
        .filter(|a| requested_id.as_deref().is_none_or(|id| a.id == id))
        .collect();

    let capabilities: Vec<Value> = filtered
        .into_iter()
        .map(|a| {
            // Only report cooldown if it is still in the future.
            let cooldown_until = a.limited_until.as_deref().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .filter(|dt| dt.with_timezone(&Utc) > now)
                    .map(|dt| dt.to_rfc3339())
            });

            let (rpm, tpm) = default_rate_limits(&a.provider);

            json!({
                "accountId": a.id,
                "provider": a.provider,
                "label": a.name,
                "tier": infer_tier(&a.provider),
                "rateLimits": {
                    "rpm": rpm,
                    "tpm": tpm,
                },
                "successRate": 1.0,
                "cooldownUntil": cooldown_until,
            })
        })
        .collect();

    Ok(json!({ "capabilities": capabilities }))
}

fn infer_tier(provider: &str) -> &'static str {
    match provider {
        "claude" => "unknown",
        "codex" => "pay-per-use",
        "cursor" => "unknown",
        _ => "unknown",
    }
}

fn default_rate_limits(provider: &str) -> (Option<u32>, Option<u32>) {
    match provider {
        "claude" => (Some(50), Some(40_000)),
        "codex" => (Some(500), Some(150_000)),
        "cursor" => (Some(100), None),
        _ => (None, None),
    }
}

// ─── gci.generate ─────────────────────────────────────────────────────────────

/// `gci.generate` — generate `~/.claude/CLAUDE.md` from questionnaire.
///
/// Params: `{ "answers": { QuestionnaireAnswers } }`
pub async fn generate_gci(params: Value, _ctx: &AppContext) -> Result<Value> {
    let answers: gci_generator::QuestionnaireAnswers = serde_json::from_value(
        params
            .get("answers")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing field: answers"))?,
    )
    .context("invalid questionnaire answers")?;

    let result = gci_generator::GciGenerator::generate_claude_md(&answers).await?;

    Ok(json!({
        "path": result.path.to_string_lossy(),
        "content": result.content,
        "backedUp": result.backed_up,
        "backupPath": result.backup_path.map(|p| p.to_string_lossy().to_string()),
    }))
}

// ─── gci.generateCodex ────────────────────────────────────────────────────────

/// `gci.generateCodex` — generate `~/.codex/AGENTS.md` from questionnaire.
///
/// Params: `{ "answers": { QuestionnaireAnswers } }`
pub async fn generate_codex_md(params: Value, _ctx: &AppContext) -> Result<Value> {
    let answers: gci_generator::QuestionnaireAnswers = serde_json::from_value(
        params
            .get("answers")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing field: answers"))?,
    )
    .context("invalid questionnaire answers")?;

    let result = gci_generator::GciGenerator::generate_codex_md(&answers).await?;

    Ok(json!({
        "path": result.path.to_string_lossy(),
        "content": result.content,
        "backedUp": result.backed_up,
        "backupPath": result.backup_path.map(|p| p.to_string_lossy().to_string()),
    }))
}

// ─── gci.generateCursor ───────────────────────────────────────────────────────

/// `gci.generateCursor` — generate `~/.cursor/rules` from questionnaire.
///
/// Params: `{ "answers": { QuestionnaireAnswers } }`
pub async fn generate_cursor_rules(params: Value, _ctx: &AppContext) -> Result<Value> {
    let answers: gci_generator::QuestionnaireAnswers = serde_json::from_value(
        params
            .get("answers")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing field: answers"))?,
    )
    .context("invalid questionnaire answers")?;

    let result = gci_generator::GciGenerator::generate_cursor_rules(&answers).await?;

    Ok(json!({
        "path": result.path.to_string_lossy(),
        "content": result.content,
        "backedUp": result.backed_up,
        "backupPath": result.backup_path.map(|p| p.to_string_lossy().to_string()),
    }))
}

// ─── repo.bootstrapAid ────────────────────────────────────────────────────────

/// `repo.bootstrapAid` — bootstrap `.claude/` for a repo with no existing AI config.
///
/// Params: `{ "repoPath": "/absolute/path/to/repo" }`
pub async fn bootstrap_aid(params: Value, ctx: &AppContext) -> Result<Value> {
    let repo_path_str = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: repoPath"))?;

    let repo_path = std::path::Path::new(repo_path_str);
    if !repo_path.exists() {
        anyhow::bail!("REPO_NOT_FOUND: {repo_path_str}");
    }

    let profile = load_or_synthesise_profile(repo_path_str, ctx).await;

    let result = aid_bootstrapper::bootstrap_aid(repo_path, &profile).await?;

    Ok(json!({
        "claudeDir": result.claude_dir.to_string_lossy(),
        "visionPath": result.vision_path.to_string_lossy(),
        "featuresPath": result.features_path.to_string_lossy(),
        "created": true,
    }))
}

/// `repo.checkAid` — return whether a repo already has `.claude/` configured.
///
/// Params: `{ "repoPath": "..." }`
pub async fn check_aid(params: Value, _ctx: &AppContext) -> Result<Value> {
    let repo_path_str = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: repoPath"))?;

    let repo_path = std::path::Path::new(repo_path_str);
    let has_aid = !aid_bootstrapper::check_repo_for_aid(repo_path);

    Ok(json!({ "hasAid": has_aid, "repoPath": repo_path_str }))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Load a stored `RepoProfile` via the repo_intelligence storage module.
/// Falls back to a minimal synthetic profile if none exists or the scan table
/// is not yet populated.
async fn load_or_synthesise_profile(
    repo_path: &str,
    ctx: &AppContext,
) -> crate::repo_intelligence::RepoProfile {
    use crate::repo_intelligence::profile::{CodeConventions, PrimaryLanguage, RepoProfile};

    match crate::repo_intelligence::storage::load(ctx.storage.pool(), repo_path).await {
        Ok(Some(profile)) => profile,
        _ => RepoProfile {
            repo_path: repo_path.to_string(),
            primary_lang: PrimaryLanguage::Unknown,
            secondary_langs: Vec::new(),
            frameworks: Vec::new(),
            build_tools: Vec::new(),
            conventions: CodeConventions::default(),
            monorepo: false,
            confidence: 0.0,
            scanned_at: Utc::now().to_rfc3339(),
        },
    }
}
