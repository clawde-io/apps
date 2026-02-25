use crate::{security, telemetry::TelemetryEvent, AppContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct CreateParams {
    provider: String,
    #[serde(rename = "repoPath")]
    repo_path: String,
    title: Option<String>,
    /// Optional permission scopes. If omitted or empty, all permissions are granted.
    /// Valid scopes: "file_read", "file_write", "shell_exec", "git".
    permissions: Option<Vec<String>>,
    /// Optional initial message for provider auto-routing.
    /// Only used when `provider = "auto"`. Not stored in the database.
    #[serde(rename = "initialMessage")]
    initial_message: Option<String>,
}

#[derive(Deserialize)]
struct SessionIdParams {
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Deserialize)]
struct SendMessageParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    content: String,
}

#[derive(Deserialize)]
struct GetMessagesParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    limit: Option<i64>,
    before: Option<String>,
}

#[derive(Deserialize)]
struct SetProviderParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    provider: String,
}

#[derive(Deserialize)]
struct SetModeParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    /// GCI mode: NORMAL | LEARN | STORM | FORGE | CRUNCH
    mode: String,
}

/// Valid provider names — must match ProviderType.name in clawd_proto.
const VALID_PROVIDERS: &[&str] = &["claude", "codex", "cursor", "auto"];

pub async fn create(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: CreateParams = serde_json::from_value(params)?;
    let title = p.title.unwrap_or_else(|| "New Session".to_string());

    // Strict provider name validation — return -32602 (invalid params) for unknown.
    if !VALID_PROVIDERS.contains(&p.provider.as_str()) {
        anyhow::bail!(
            "invalid type: unknown provider '{}' — must be one of: {}",
            p.provider,
            VALID_PROVIDERS.join(", ")
        );
    }

    // Validate repo_path exists before creating the session record.
    if !std::path::Path::new(&p.repo_path).exists() {
        anyhow::bail!("REPO_NOT_FOUND: repo path does not exist: {}", p.repo_path);
    }

    // DC.T41: check for data-dir overlap and .clawd/ injection
    security::check_repo_path_safety(
        std::path::Path::new(&p.repo_path),
        &ctx.config.data_dir,
    )?;

    // Validate permission scope names if provided
    if let Some(ref perms) = p.permissions {
        const VALID_SCOPES: &[&str] = &["file_read", "file_write", "shell_exec", "git"];
        for scope in perms {
            if !VALID_SCOPES.contains(&scope.as_str()) {
                anyhow::bail!(
                    "invalid type: unknown permission scope '{}' — must be one of: {}",
                    scope,
                    VALID_SCOPES.join(", ")
                );
            }
        }
    }

    let session = ctx
        .session_manager
        .create(
            &p.provider,
            &p.repo_path,
            &title,
            ctx.config.max_sessions,
            p.permissions,
            p.initial_message.as_deref(),
        )
        .await?;
    // D64.T16: start watching manifest files for version bumps in this repo.
    ctx.version_watcher
        .watch(std::path::Path::new(&p.repo_path))
        .await;
    ctx.telemetry
        .send(TelemetryEvent::new("session.start").with_provider(&p.provider));
    Ok(serde_json::to_value(session)?)
}

pub async fn list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let sessions = ctx.session_manager.list().await?;
    Ok(json!(sessions))
}

pub async fn get(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SessionIdParams = serde_json::from_value(params)?;
    let session = ctx.session_manager.get(&p.session_id).await?;
    Ok(serde_json::to_value(session)?)
}

pub async fn delete(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SessionIdParams = serde_json::from_value(params)?;
    ctx.session_manager.delete(&p.session_id).await?;
    ctx.telemetry.send(TelemetryEvent::new("session.end"));
    Ok(json!({}))
}

pub async fn send_message(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SendMessageParams = serde_json::from_value(params)?;
    let message = ctx
        .session_manager
        .send_message(&p.session_id, &p.content, ctx)
        .await?;
    Ok(serde_json::to_value(message)?)
}

pub async fn get_messages(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: GetMessagesParams = serde_json::from_value(params)?;
    let limit = p.limit.unwrap_or(50).min(200); // cap at 200 to prevent DoS
    let messages = ctx
        .session_manager
        .get_messages(&p.session_id, limit, p.before.as_deref())
        .await?;
    Ok(json!(messages))
}

pub async fn pause(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SessionIdParams = serde_json::from_value(params)?;
    ctx.session_manager.pause(&p.session_id).await?;
    Ok(json!({}))
}

pub async fn resume(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SessionIdParams = serde_json::from_value(params)?;
    ctx.session_manager.resume(&p.session_id).await?;
    Ok(json!({}))
}

pub async fn cancel(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SessionIdParams = serde_json::from_value(params)?;
    ctx.session_manager.cancel(&p.session_id).await?;
    Ok(json!({}))
}

pub async fn set_provider(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SetProviderParams = serde_json::from_value(params)?;
    ctx.session_manager
        .set_provider(&p.session_id, &p.provider)
        .await?;
    Ok(json!({}))
}

/// `session.setMode` — set the GCI mode on a session.
///
/// Params: `{ sessionId: string, mode: "NORMAL" | "LEARN" | "STORM" | "FORGE" | "CRUNCH" }`
/// Returns: `{}`
/// Push event: `session.modeChanged { sessionId, mode, previousMode }`
pub async fn set_mode(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SetModeParams = serde_json::from_value(params)?;

    const VALID_MODES: &[&str] = &["NORMAL", "LEARN", "STORM", "FORGE", "CRUNCH"];
    if !VALID_MODES.contains(&p.mode.as_str()) {
        anyhow::bail!(
            "invalid type: unknown mode '{}' — must be one of: {}",
            p.mode,
            VALID_MODES.join(", ")
        );
    }

    // Fetch current mode before updating (for the push event).
    let session = ctx
        .storage
        .get_session(&p.session_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("SESSION_NOT_FOUND: {}", p.session_id))?;

    let previous_mode = session.mode.clone();
    ctx.storage
        .set_session_mode(&p.session_id, &p.mode)
        .await?;

    ctx.broadcaster.broadcast(
        "session.modeChanged",
        serde_json::json!({
            "sessionId": p.session_id,
            "mode": p.mode,
            "previousMode": previous_mode,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    );

    Ok(json!({}))
}
