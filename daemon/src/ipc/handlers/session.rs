use crate::{telemetry::TelemetryEvent, AppContext};
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

/// Valid provider names — must match ProviderType.name in clawd_proto.
const VALID_PROVIDERS: &[&str] = &["claude", "codex", "cursor"];

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
        )
        .await?;
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
