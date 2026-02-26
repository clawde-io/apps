use crate::{security, telemetry::TelemetryEvent, AppContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

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
    /// Optional session ID to inherit context from (Sprint BB PV.13).
    ///
    /// When set, the new session receives a system context primer built from
    /// the source session: last 3 AI turns, diff window (files changed since
    /// that session started), and active task IDs.  This lets follow-up
    /// sessions continue where the previous one left off.
    #[serde(rename = "inheritFrom")]
    inherit_from: Option<String>,
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
    security::check_repo_path_safety(std::path::Path::new(&p.repo_path), &ctx.config.data_dir)?;

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

    // Sprint BB PV.13: build context primer from a prior session if requested.
    // The primer is prepended to the title so the runner can inject it as an
    // initial system message on the first turn.  We use the title channel
    // (non-persistent) rather than a new DB column to keep the migration surface
    // minimal; a future sprint can promote this to a dedicated field.
    let effective_initial_message =
        build_inherit_primer(ctx, p.inherit_from.as_deref(), p.initial_message.as_deref()).await;

    let session = ctx
        .session_manager
        .create(
            &p.provider,
            &p.repo_path,
            &title,
            ctx.config.max_sessions,
            p.permissions,
            effective_initial_message.as_deref(),
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
    ctx.storage.set_session_mode(&p.session_id, &p.mode).await?;

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

// ─── Context inheritance (Sprint BB PV.13) ────────────────────────────────────

/// Build an `initial_message` primer from a prior session's context.
///
/// Fetches the last 3 AI turns, active task IDs from the source session, then
/// assembles them as a compact context block prepended to the original message.
/// If `inherit_from` is None, returns the original `initial_message` unchanged.
async fn build_inherit_primer(
    ctx: &AppContext,
    inherit_from: Option<&str>,
    original_message: Option<&str>,
) -> Option<String> {
    let source_id = match inherit_from {
        Some(id) if !id.is_empty() => id,
        _ => return original_message.map(|s| s.to_string()),
    };

    // Fetch last 3 assistant messages from the source session.
    let messages = match ctx.storage.list_messages(source_id, 50, None).await {
        Ok(msgs) => msgs,
        Err(_) => return original_message.map(|s| s.to_string()),
    };

    // Collect to Vec first so we can call .iter().rev() (Filter doesn't impl ExactSizeIterator).
    let assistant_msgs: Vec<_> = messages
        .iter()
        .filter(|m| m.role == "assistant" && m.status == "done")
        .collect();
    let last_turns: Vec<String> = assistant_msgs
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(|m| {
            let preview = m.content.chars().take(400).collect::<String>();
            format!("[turn] {preview}")
        })
        .collect();

    // Fetch active task IDs from the task queue.
    use crate::tasks::storage::TaskListParams;
    let task_ids: Vec<String> = ctx
        .task_storage
        .list_tasks(&TaskListParams {
            status: Some("active".to_string()),
            ..Default::default()
        })
        .await
        .unwrap_or_default()
        .iter()
        .map(|t| t.id.clone())
        .collect();

    if last_turns.is_empty() && task_ids.is_empty() {
        return original_message.map(|s| s.to_string());
    }

    let mut primer = format!(
        "[Context inherited from session {}]\n",
        &source_id[..8.min(source_id.len())]
    );

    if !task_ids.is_empty() {
        primer.push_str(&format!("Active tasks: {}\n", task_ids.join(", ")));
    }
    if !last_turns.is_empty() {
        primer.push_str("Recent AI turns:\n");
        for turn in &last_turns {
            primer.push_str(&format!("  {turn}\n"));
        }
    }

    if let Some(orig) = original_message {
        primer.push('\n');
        primer.push_str(orig);
    }

    Some(primer)
}

// ── Sprint CC AM.4 — Attention Map RPC ────────────────────────────────────────

/// `session.attention_map` — top-N files by combined attention score for a session.
pub async fn attention_map(params: Value, ctx: &AppContext) -> Result<Value> {
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing sessionId"))?;
    let top_n = params
        .get("topN")
        .and_then(|v| v.as_u64())
        .unwrap_or(20)
        .min(100) as i64;

    let rows = sqlx::query(
        "SELECT file_path, read_count, write_count, mention_count,
                (read_count + write_count * 2 + mention_count) AS attention_score,
                last_accessed_at
         FROM session_file_attention
         WHERE session_id = ?
         ORDER BY attention_score DESC
         LIMIT ?",
    )
    .bind(session_id)
    .bind(top_n)
    .fetch_all(ctx.storage.pool())
    .await?;

    let files: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "filePath": r.get::<String, _>("file_path"),
                "readCount": r.get::<i64, _>("read_count"),
                "writeCount": r.get::<i64, _>("write_count"),
                "mentionCount": r.get::<i64, _>("mention_count"),
                "attentionScore": r.get::<i64, _>("attention_score"),
                "lastAccessedAt": r.get::<i64, _>("last_accessed_at"),
            })
        })
        .collect();

    Ok(json!({ "sessionId": session_id, "files": files }))
}

// ── Sprint CC IE.4 — Intent Summary RPC ──────────────────────────────────────

/// `session.intent_summary` — intent vs execution diff for a session.
pub async fn intent_summary(params: Value, ctx: &AppContext) -> Result<Value> {
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing sessionId"))?;

    let row = sqlx::query(
        "SELECT intent_json, execution_json, intent_divergence_score FROM sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(ctx.storage.pool())
    .await?;

    match row {
        None => anyhow::bail!("session not found"),
        Some(r) => {
            let intent: Option<serde_json::Value> = r
                .get::<Option<String>, _>("intent_json")
                .and_then(|s| serde_json::from_str(&s).ok());
            let execution: Option<serde_json::Value> = r
                .get::<Option<String>, _>("execution_json")
                .and_then(|s| serde_json::from_str(&s).ok());
            let divergence = r.get::<Option<f64>, _>("intent_divergence_score");

            Ok(json!({
                "sessionId": session_id,
                "intent": intent,
                "execution": execution,
                "divergenceScore": divergence,
            }))
        }
    }
}
