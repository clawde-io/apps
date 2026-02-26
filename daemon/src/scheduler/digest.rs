//! Sprint EE DD.1/DD.3 — Daily digest generator.
//!
//! Runs a scheduled job at ~6pm local time to summarize the day's sessions
//! and optionally send a push notification to registered mobile devices.

use anyhow::Result;
use serde_json::{json, Value};
use sqlx::SqlitePool;

/// Digest metrics for a single day.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DigestMetrics {
    pub sessions_run: i64,
    pub tasks_completed: i64,
    pub tasks_in_progress: i64,
    pub top_files: Vec<String>,
    pub eval_avg: f64,
    pub velocity: serde_json::Map<String, Value>,
}

/// A session entry in the daily digest.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DigestEntry {
    pub session_id: String,
    pub session_title: Option<String>,
    pub provider: String,
    pub messages_count: i64,
    pub tasks_completed: i64,
    pub files_changed: Vec<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
}

/// The daily digest report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyDigest {
    pub date: String,
    pub metrics: DigestMetrics,
    pub sessions: Vec<DigestEntry>,
}

/// Generate the daily digest for today.
pub async fn generate_today(pool: &SqlitePool) -> Result<DailyDigest> {
    use sqlx::Row as _;

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Count sessions started today
    let sessions_run: i64 = sqlx::query(
        "SELECT COUNT(*) as cnt FROM sessions WHERE date(created_at) = ?",
    )
    .bind(&today)
    .fetch_one(pool)
    .await
    .map(|r| r.get::<i64, _>("cnt"))
    .unwrap_or(0);

    // Get session entries for today
    let session_rows = sqlx::query(
        "SELECT id, provider, status, created_at, updated_at
         FROM sessions
         WHERE date(created_at) = ?
         ORDER BY created_at DESC
         LIMIT 20",
    )
    .bind(&today)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let sessions: Vec<DigestEntry> = session_rows
        .iter()
        .map(|r| {
            let session_id: String = r.get("id");
            let provider: String = r.get::<Option<String>, _>("provider")
                .unwrap_or_else(|| "claude".to_string());
            let status: String = r.get::<Option<String>, _>("status")
                .unwrap_or_else(|| "idle".to_string());
            let started_at: String = r.get::<Option<String>, _>("created_at")
                .unwrap_or_default();
            let ended_at: Option<String> = r.get("updated_at");

            DigestEntry {
                session_id,
                session_title: None,
                provider,
                messages_count: 0, // would join messages table in production
                tasks_completed: if status == "completed" { 1 } else { 0 },
                files_changed: vec![],
                started_at,
                ended_at,
            }
        })
        .collect();

    let tasks_completed = sessions.iter().map(|s| s.tasks_completed).sum();
    let tasks_in_progress = sessions.iter().filter(|s| {
        s.ended_at.is_none() || s.session_id.contains("running")
    }).count() as i64;

    let metrics = DigestMetrics {
        sessions_run,
        tasks_completed,
        tasks_in_progress,
        top_files: vec![],
        eval_avg: 0.0,
        velocity: serde_json::Map::new(),
    };

    Ok(DailyDigest {
        date: today,
        metrics,
        sessions,
    })
}

/// Build the `digest.today` RPC response.
pub async fn today_response(pool: &SqlitePool) -> Result<Value> {
    let digest = generate_today(pool).await?;
    Ok(json!({
        "date": digest.date,
        "metrics": {
            "sessionsRun": digest.metrics.sessions_run,
            "tasksCompleted": digest.metrics.tasks_completed,
            "tasksInProgress": digest.metrics.tasks_in_progress,
            "topFiles": digest.metrics.top_files,
            "evalAvg": digest.metrics.eval_avg,
            "velocity": digest.metrics.velocity,
        },
        "sessions": digest.sessions.iter().map(|s| json!({
            "sessionId": s.session_id,
            "sessionTitle": s.session_title,
            "provider": s.provider,
            "messagesCount": s.messages_count,
            "tasksCompleted": s.tasks_completed,
            "filesChanged": s.files_changed,
            "startedAt": s.started_at,
            "endedAt": s.ended_at,
        })).collect::<Vec<_>>(),
    }))
}
