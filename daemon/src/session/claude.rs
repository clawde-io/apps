use super::events::EventLog;
use super::runner::Runner;
use crate::{ipc::event::EventBroadcaster, storage::Storage};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::RwLock,
};
use tracing::{debug, warn};

// ─── Claude stream-json output types ─────────────────────────────────────────

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum ClaudeEvent {
    /// Text content from the assistant
    Assistant { message: AssistantMessage },
    /// Tool use request (informational when --dangerously-skip-permissions)
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// Tool result echoed back by claude
    #[serde(rename = "tool_result")]
    ToolResult { tool_use_id: String, content: Value },
    /// Final result of the turn
    Result {
        subtype: String,
        result: Option<String>,
        is_error: Option<bool>,
    },
    /// Startup event — contains claude's own session_id for --resume
    System {
        subtype: Option<String>,
        session_id: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct AssistantMessage {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

// ─── Runner ───────────────────────────────────────────────────────────────────

pub struct ClaudeCodeRunner {
    session_id: String,
    repo_path: String,
    storage: Arc<Storage>,
    /// Append-only JSONL log of all raw claude events for this session.
    event_log: EventLog,
    broadcaster: Arc<EventBroadcaster>,
    /// Claude's own session ID, captured from the first System event.
    /// Used to pass --resume on every subsequent turn so conversation
    /// history is preserved across process restarts.
    claude_session_id: RwLock<Option<String>>,
    paused: Arc<std::sync::atomic::AtomicBool>,
}

impl ClaudeCodeRunner {
    pub fn new(
        session_id: String,
        repo_path: String,
        storage: Arc<Storage>,
        data_dir: std::path::PathBuf,
        broadcaster: Arc<EventBroadcaster>,
    ) -> Arc<Self> {
        Arc::new(Self {
            event_log: EventLog::new(&data_dir, &session_id),
            session_id,
            repo_path,
            storage,
            broadcaster,
            claude_session_id: RwLock::new(None),
            paused: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Spawn `claude` for one turn and drive the event loop to completion.
    ///
    /// On the first turn, claude is started without `--resume`. The System
    /// event returns claude's own session_id which is stored in
    /// `self.claude_session_id`. Every subsequent turn passes
    /// `--resume <id>` so claude reloads the conversation history.
    ///
    /// Callers must run this inside `tokio::spawn` — it blocks until the
    /// turn completes (i.e., the claude process exits).
    pub async fn run_turn(&self, content: &str) -> Result<()> {
        let claude_sid = self.claude_session_id.read().await.clone();

        let mut cmd = Command::new("claude");
        cmd.args([
            "--output-format",
            "stream-json",
            "--dangerously-skip-permissions",
            "-p",
            content,
        ]);
        if let Some(ref sid) = claude_sid {
            cmd.args(["--resume", sid]);
        }

        let mut child = cmd
            .current_dir(&self.repo_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .spawn()
            .context("failed to spawn `claude` — is it installed and on PATH?")?;

        let stdout = child.stdout.take().context("no stdout")?;
        let stderr = child.stderr.take().context("no stderr")?;

        // Drain stderr to avoid blocking the process
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!(target: "claude_stderr", "{}", line);
            }
        });

        self.event_loop(stdout, child).await
    }

    async fn event_loop(
        &self,
        stdout: tokio::process::ChildStdout,
        mut child: Child,
    ) -> Result<()> {
        let mut lines = BufReader::new(stdout).lines();
        let mut current_message_id: Option<String> = None;
        let mut current_content = String::new();

        self.storage
            .update_session_status(&self.session_id, "running")
            .await?;
        self.broadcaster.broadcast(
            "session.statusChanged",
            json!({ "sessionId": self.session_id, "status": "running" }),
        );

        while let Some(line) = lines.next_line().await? {
            debug!(session = %self.session_id, event = %line, "claude event");

            // Write every raw event to the session's JSONL log
            if let Ok(raw) = serde_json::from_str::<Value>(&line) {
                let _ = self.event_log.append(&raw).await;
            }

            let event: ClaudeEvent = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => {
                    warn!(line = %line, "unparseable claude event");
                    continue;
                }
            };

            match event {
                // ── Capture claude's session ID for subsequent --resume ──────
                ClaudeEvent::System { session_id, .. } => {
                    if let Some(sid) = session_id {
                        *self.claude_session_id.write().await = Some(sid);
                    }
                }

                // ── Streaming assistant text ─────────────────────────────────
                ClaudeEvent::Assistant { message } => {
                    let text = message
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("");

                    if let Some(ref msg_id) = current_message_id {
                        current_content = text.clone();
                        self.storage
                            .update_message_content(msg_id, &current_content, "streaming")
                            .await?;
                        self.broadcaster.broadcast(
                            "session.messageUpdated",
                            json!({
                                "sessionId": self.session_id,
                                "messageId": msg_id,
                                "content": current_content,
                                "status": "streaming"
                            }),
                        );
                    } else {
                        let msg = self
                            .storage
                            .create_message(&self.session_id, "assistant", &text, "streaming")
                            .await?;
                        self.storage
                            .increment_message_count(&self.session_id)
                            .await?;
                        current_message_id = Some(msg.id.clone());
                        current_content = text.clone();
                        self.broadcaster.broadcast(
                            "session.messageCreated",
                            json!({
                                "sessionId": self.session_id,
                                "message": {
                                    "id": msg.id,
                                    "sessionId": self.session_id,
                                    "role": "assistant",
                                    "content": text,
                                    "status": "streaming",
                                    "createdAt": msg.created_at
                                }
                            }),
                        );
                    }
                }

                // ── Tool use — auto-approve (dangerously-skip-permissions) ───
                //
                // Claude executes tools itself; this event is informational.
                // We record it for the UI and complete it immediately — the
                // session never enters "waiting" status.
                ClaudeEvent::ToolUse { id: _, name, input } => {
                    // Finalize any in-progress assistant message first
                    if let Some(ref msg_id) = current_message_id {
                        self.storage
                            .update_message_content(msg_id, &current_content, "done")
                            .await?;
                        self.broadcaster.broadcast(
                            "session.messageUpdated",
                            json!({
                                "sessionId": self.session_id,
                                "messageId": msg_id,
                                "content": current_content,
                                "status": "done"
                            }),
                        );
                        current_message_id = None;
                    }

                    let tool_msg = self
                        .storage
                        .create_message(&self.session_id, "tool", "", "done")
                        .await?;
                    let input_str = serde_json::to_string(&input).unwrap_or_default();
                    let tool_call = self
                        .storage
                        .create_tool_call(&self.session_id, &tool_msg.id, &name, &input_str)
                        .await?;

                    self.broadcaster.broadcast(
                        "session.toolCallCreated",
                        json!({
                            "sessionId": self.session_id,
                            "toolCall": {
                                "id": tool_call.id,
                                "messageId": tool_msg.id,
                                "name": name,
                                "input": input,
                                "status": "running",
                                "createdAt": tool_call.created_at
                            }
                        }),
                    );

                    // Auto-complete — no user approval required
                    self.storage
                        .complete_tool_call(&tool_call.id, Some("auto-approved"), "done")
                        .await?;
                    self.broadcaster.broadcast(
                        "session.toolCallUpdated",
                        json!({
                            "sessionId": self.session_id,
                            "toolCallId": tool_call.id,
                            "status": "done",
                            "output": "auto-approved"
                        }),
                    );
                }

                // ── Turn complete ────────────────────────────────────────────
                ClaudeEvent::Result { is_error, .. } => {
                    if let Some(ref msg_id) = current_message_id {
                        self.storage
                            .update_message_content(msg_id, &current_content, "done")
                            .await?;
                        self.broadcaster.broadcast(
                            "session.messageUpdated",
                            json!({
                                "sessionId": self.session_id,
                                "messageId": msg_id,
                                "content": current_content,
                                "status": "done"
                            }),
                        );
                        current_message_id = None;
                    }

                    let final_status = if is_error.unwrap_or(false) {
                        "error"
                    } else {
                        "idle"
                    };
                    self.storage
                        .update_session_status(&self.session_id, final_status)
                        .await?;
                    self.broadcaster.broadcast(
                        "session.statusChanged",
                        json!({ "sessionId": self.session_id, "status": final_status }),
                    );
                }

                ClaudeEvent::ToolResult { .. } | ClaudeEvent::Unknown => {}
            }
        }

        child.wait().await?;
        Ok(())
    }
}

#[async_trait]
impl Runner for ClaudeCodeRunner {
    async fn send(&self, _content: &str) -> Result<()> {
        // Sending is driven by run_turn() called directly from SessionManager.
        Ok(())
    }

    async fn pause(&self) -> Result<()> {
        self.paused
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn resume(&self) -> Result<()> {
        self.paused
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        Ok(())
    }
}
