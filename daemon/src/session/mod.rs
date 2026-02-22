pub mod claude;
pub mod events;
pub mod runner;

use crate::{ipc::event::EventBroadcaster, storage::Storage, AppContext};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tracing::info;

use claude::ClaudeCodeRunner;
use runner::{Runner, ToolDecision};

// ─── View types (matching @clawde/proto) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub provider: String,
    pub repo_path: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageView {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

// ─── In-memory session handle ─────────────────────────────────────────────────

struct SessionHandle {
    runner: Arc<ClaudeCodeRunner>,
}

// ─── Manager ─────────────────────────────────────────────────────────────────

pub struct SessionManager {
    storage: Arc<Storage>,
    broadcaster: Arc<EventBroadcaster>,
    data_dir: PathBuf,
    /// In-memory runners for active sessions
    handles: RwLock<HashMap<String, Arc<SessionHandle>>>,
}

impl SessionManager {
    pub fn new(
        storage: Arc<Storage>,
        broadcaster: Arc<EventBroadcaster>,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            storage,
            broadcaster,
            data_dir,
            handles: RwLock::new(HashMap::new()),
        }
    }

    pub async fn active_count(&self) -> usize {
        self.handles.read().await.len()
    }

    // ─── CRUD ────────────────────────────────────────────────────────────────

    pub async fn create(
        &self,
        provider: &str,
        repo_path: &str,
        title: &str,
    ) -> Result<SessionView> {
        // Validate provider
        match provider {
            "claude-code" | "codex" | "cursor" => {}
            _ => return Err(anyhow::anyhow!("unknown provider: {}", provider)),
        }

        let row = self
            .storage
            .create_session(provider, repo_path, title)
            .await?;
        info!(id = %row.id, provider = %provider, "session created");

        let view = row_to_view(row);
        self.broadcaster.broadcast(
            "session.statusChanged",
            json!({ "sessionId": view.id, "status": view.status }),
        );
        Ok(view)
    }

    pub async fn list(&self) -> Result<Vec<SessionView>> {
        let rows = self.storage.list_sessions().await?;
        Ok(rows.into_iter().map(row_to_view).collect())
    }

    pub async fn get(&self, session_id: &str) -> Result<SessionView> {
        self.storage
            .get_session(session_id)
            .await?
            .map(row_to_view)
            .context("SESSION_NOT_FOUND")
    }

    pub async fn delete(&self, session_id: &str) -> Result<()> {
        // Stop runner if active
        if let Some(handle) = self.handles.write().await.remove(session_id) {
            let _ = handle.runner.stop().await;
        }
        self.storage.delete_session(session_id).await?;
        info!(id = %session_id, "session deleted");
        Ok(())
    }

    pub async fn pause(&self, session_id: &str) -> Result<()> {
        self.storage
            .update_session_status(session_id, "paused")
            .await?;
        if let Some(handle) = self.handles.read().await.get(session_id) {
            handle.runner.pause().await?;
        }
        self.broadcaster.broadcast(
            "session.statusChanged",
            json!({ "sessionId": session_id, "status": "paused" }),
        );
        Ok(())
    }

    pub async fn resume(&self, session_id: &str) -> Result<()> {
        self.storage
            .update_session_status(session_id, "idle")
            .await?;
        if let Some(handle) = self.handles.read().await.get(session_id) {
            handle.runner.resume().await?;
        }
        self.broadcaster.broadcast(
            "session.statusChanged",
            json!({ "sessionId": session_id, "status": "idle" }),
        );
        Ok(())
    }

    // ─── Messages ─────────────────────────────────────────────────────────────

    pub async fn send_message(
        &self,
        session_id: &str,
        content: &str,
        _ctx: &AppContext,
    ) -> Result<MessageView> {
        // Ensure session exists
        let session_row = self
            .storage
            .get_session(session_id)
            .await?
            .context("SESSION_NOT_FOUND")?;

        // Persist user message
        let msg = self
            .storage
            .create_message(session_id, "user", content, "done")
            .await?;
        self.storage.increment_message_count(session_id).await?;

        let msg_view = msg_row_to_view(msg.clone());
        self.broadcaster.broadcast(
            "session.messageCreated",
            json!({
                "sessionId": session_id,
                "message": msg_view
            }),
        );

        // Get or create a runner for this session
        let runner = {
            let mut handles = self.handles.write().await;
            if let Some(h) = handles.get(session_id) {
                h.runner.clone()
            } else {
                let runner = ClaudeCodeRunner::new(
                    session_id.to_string(),
                    session_row.repo_path.clone(),
                    self.storage.clone(),
                    self.data_dir.clone(),
                    self.broadcaster.clone(),
                );
                let runner_clone = runner.clone();
                runner_clone.start().await?;
                let handle = Arc::new(SessionHandle {
                    runner: runner.clone(),
                });
                handles.insert(session_id.to_string(), handle);
                runner
            }
        };

        // Send message to runner
        runner.send(content).await?;

        Ok(msg_view)
    }

    pub async fn get_messages(
        &self,
        session_id: &str,
        limit: i64,
        before: Option<&str>,
    ) -> Result<Vec<MessageView>> {
        let rows = self
            .storage
            .list_messages(session_id, limit, before)
            .await?;
        Ok(rows.into_iter().map(msg_row_to_view).collect())
    }

    // ─── Tool approval ────────────────────────────────────────────────────────

    pub async fn approve_tool(&self, session_id: &str, tool_call_id: &str) -> Result<()> {
        let handle = self
            .handles
            .read()
            .await
            .get(session_id)
            .cloned()
            .context("SESSION_NOT_FOUND")?;
        handle
            .runner
            .resolve_tool(tool_call_id, ToolDecision::Approved)
            .await
    }

    pub async fn reject_tool(&self, session_id: &str, tool_call_id: &str) -> Result<()> {
        let handle = self
            .handles
            .read()
            .await
            .get(session_id)
            .cloned()
            .context("SESSION_NOT_FOUND")?;
        handle
            .runner
            .resolve_tool(tool_call_id, ToolDecision::Rejected)
            .await
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn row_to_view(row: crate::storage::SessionRow) -> SessionView {
    SessionView {
        id: row.id,
        provider: row.provider,
        repo_path: row.repo_path,
        title: row.title,
        status: row.status,
        created_at: row.created_at,
        updated_at: row.updated_at,
        message_count: row.message_count,
    }
}

fn msg_row_to_view(row: crate::storage::MessageRow) -> MessageView {
    MessageView {
        id: row.id,
        session_id: row.session_id,
        role: row.role,
        content: row.content,
        status: row.status,
        created_at: row.created_at,
    }
}
