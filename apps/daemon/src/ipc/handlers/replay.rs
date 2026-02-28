//! Sprint DD SR.1/SR.2/SR.4/SR.5 — `session.export`, `session.import`, `session.replay`.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

/// Bundle schema version for forward-compat checks.
const BUNDLE_VERSION: &str = "1.0";

/// `session.export` — serialize a session to a portable `.clawbundle` (gzipped JSON).
pub async fn export(params: Value, ctx: &AppContext) -> Result<Value> {
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("sessionId required"))?;

    // Load session metadata.
    let session = ctx.session_manager.get(session_id).await?;

    // Load messages.
    let messages = ctx
        .session_manager
        .get_messages(session_id, 10_000, None)
        .await?;

    let bundle = json!({
        "version": BUNDLE_VERSION,
        "exportedAt": chrono::Utc::now().to_rfc3339(),
        "session": {
            "id": session.id,
            "provider": session.provider,
            "title": session.title,
            "repoPath": session.repo_path,
            "createdAt": session.created_at,
        },
        "messages": messages.iter().map(|m| json!({
            "id": m.id,
            "role": m.role,
            "content": m.content,
            "status": m.status,
            "createdAt": m.created_at,
        })).collect::<Vec<_>>(),
    });

    // Gzip-compress the JSON.
    let json_bytes = serde_json::to_vec(&bundle)?;
    let compressed = gzip_compress(&json_bytes)?;
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &compressed);

    Ok(json!({
        "sessionId": session_id,
        "bundleVersion": BUNDLE_VERSION,
        "bundleB64": b64,
        "messageCount": messages.len(),
    }))
}

/// `session.import` — create a replay session from a `.clawbundle`.
pub async fn import_bundle(params: Value, ctx: &AppContext) -> Result<Value> {
    let bundle_b64 = params
        .get("bundleB64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("bundleB64 required"))?;
    let repo_path = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    // Decode + decompress.
    let compressed =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, bundle_b64)?;
    let json_bytes = gzip_decompress(&compressed)?;
    let bundle: Value = serde_json::from_slice(&json_bytes)?;

    let orig_title = bundle
        .get("session")
        .and_then(|s| s.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("Imported session");

    let title = format!("[Replay] {}", orig_title);

    // Create a new session in replay mode.
    let new_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (id, provider, repo_path, title, status, message_count)
         VALUES (?, 'replay', ?, ?, 'replay', 0)",
    )
    .bind(&new_id)
    .bind(repo_path)
    .bind(&title)
    .execute(ctx.storage.pool())
    .await?;

    // Import messages.
    if let Some(messages) = bundle.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            let msg_id = Uuid::new_v4().to_string();
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            sqlx::query(
                "INSERT INTO messages (id, session_id, role, content, status)
                 VALUES (?, ?, ?, ?, 'done')",
            )
            .bind(&msg_id)
            .bind(&new_id)
            .bind(role)
            .bind(content)
            .execute(ctx.storage.pool())
            .await?;
        }

        // Update message count.
        sqlx::query("UPDATE sessions SET message_count = ? WHERE id = ?")
            .bind(messages.len() as i64)
            .bind(&new_id)
            .execute(ctx.storage.pool())
            .await?;
    }

    ctx.broadcaster.broadcast(
        "session.statusChanged",
        json!({ "sessionId": new_id, "status": "replay" }),
    );

    Ok(json!({
        "newSessionId": new_id,
        "title": title,
        "status": "replay",
    }))
}

/// `session.replay` — step through a replay session.
///
/// Emits `session.messageCreated` push events at the requested speed so
/// the UI can "watch" the session replay in real time.
pub async fn replay(params: Value, ctx: &AppContext) -> Result<Value> {
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("sessionId required"))?;
    let speed = params
        .get("speed")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0)
        .clamp(0.1, 10.0);

    let messages = ctx
        .session_manager
        .get_messages(session_id, 10_000, None)
        .await?;

    let delay_ms = (500.0 / speed) as u64;
    let session_id_owned = session_id.to_string();
    let broadcaster = ctx.broadcaster.clone();

    tokio::spawn(async move {
        for msg in messages {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            broadcaster.broadcast(
                "session.messageCreated",
                json!({
                    "sessionId": session_id_owned,
                    "message": {
                        "id": msg.id,
                        "role": msg.role,
                        "content": msg.content,
                        "status": msg.status,
                        "createdAt": msg.created_at,
                    },
                    "replayMode": true,
                }),
            );
        }

        broadcaster.broadcast(
            "session.replayComplete",
            json!({ "sessionId": session_id_owned }),
        );
    });

    Ok(json!({
        "sessionId": session_id,
        "speed": speed,
        "status": "replaying",
    }))
}

fn gzip_compress(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

fn gzip_decompress(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}
