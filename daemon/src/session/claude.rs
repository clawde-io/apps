use super::runner::{Runner, ToolDecision};
use crate::{ipc::event::EventBroadcaster, storage::Storage};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{oneshot, RwLock},
};
use tracing::{debug, error, warn};

// ─── Claude stream-json output types ────────────────────────────────────────

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeEvent {
    /// Text content from the assistant
    Assistant {
        message: AssistantMessage,
    },
    /// Tool use request
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// Tool result (echo back from claude after we provide approval)
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Value,
    },
    /// Final result of the session turn
    Result {
        subtype: String,
        result: Option<String>,
        is_error: Option<bool>,
    },
    /// System messages (usually startup info)
    System {
        subtype: Option<String>,
        session_id: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
struct AssistantMessage {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text { text: String },
    #[serde(other)]
    Other,
}

// ─── Runner state ────────────────────────────────────────────────────────────

pub struct ClaudeCodeRunner {
    session_id: String,
    repo_path: String,
    storage: Arc<Storage>,
    data_dir: std::path::PathBuf,
    broadcaster: Arc<EventBroadcaster>,
    /// Channel to send messages into the running subprocess stdin
    stdin_tx: RwLock<Option<tokio::sync::mpsc::Sender<String>>>,
    /// Pending tool approvals: tool_call_id → oneshot sender
    tool_queue: Arc<Mutex<HashMap<String, oneshot::Sender<ToolDecision>>>>,
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
            session_id,
            repo_path,
            storage,
            data_dir,
            broadcaster,
            stdin_tx: RwLock::new(None),
            tool_queue: Arc::new(Mutex::new(HashMap::new())),
            paused: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Spawn the `claude` subprocess and start the event loop.
    pub async fn start(self: Arc<Self>) -> Result<()> {
        let mut child = Command::new("claude")
            .args([
                "--output-format",
                "stream-json",
                "--dangerously-skip-permissions",
            ])
            .current_dir(&self.repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to spawn `claude` — is it installed and on PATH?")?;

        let stdin = child.stdin.take().context("no stdin")?;
        let stdout = child.stdout.take().context("no stdout")?;
        let stderr = child.stderr.take().context("no stderr")?;

        // Channel for sending messages to stdin
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel::<String>(32);
        *self.stdin_tx.write().await = Some(stdin_tx);

        let runner = self.clone();

        // stdin writer task
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(msg) = stdin_rx.recv().await {
                if let Err(e) = stdin.write_all(msg.as_bytes()).await {
                    warn!(err = %e, "stdin write error");
                    break;
                }
                if let Err(e) = stdin.write_all(b"\n").await {
                    warn!(err = %e, "stdin write error");
                    break;
                }
            }
        });

        // stderr logger task
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!(target: "claude_stderr", "{}", line);
            }
        });

        // stdout event loop (main task)
        tokio::spawn(async move {
            if let Err(e) = runner.event_loop(stdout, child).await {
                error!(session = %runner.session_id, err = %e, "runner error");
                let _ = runner
                    .storage
                    .update_session_status(&runner.session_id, "error")
                    .await;
                runner.broadcaster.broadcast(
                    "session.statusChanged",
                    json!({ "sessionId": runner.session_id, "status": "error" }),
                );
            }
        });

        Ok(())
    }

    async fn event_loop(
        &self,
        stdout: tokio::process::ChildStdout,
        mut child: Child,
    ) -> Result<()> {
        let mut lines = BufReader::new(stdout).lines();
        // Track the current assistant message being streamed
        let mut current_message_id: Option<String> = None;
        let mut current_content = String::new();

        // Update session to "running"
        self.storage
            .update_session_status(&self.session_id, "running")
            .await?;
        self.broadcaster.broadcast(
            "session.statusChanged",
            json!({ "sessionId": self.session_id, "status": "running" }),
        );

        while let Some(line) = lines.next_line().await? {
            debug!(session = %self.session_id, event = %line, "claude event");

            let event: ClaudeEvent = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => {
                    warn!(line = %line, "unparseable claude event");
                    continue;
                }
            };

            match event {
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
                        // Streaming update
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
                        // New message
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

                ClaudeEvent::ToolUse { id: _tool_id, name, input } => {
                    // Finalize the current assistant message first
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

                    // Create a "tool" role message to hold this tool call
                    let tool_msg = self
                        .storage
                        .create_message(&self.session_id, "tool", "", "done")
                        .await?;

                    let input_str = serde_json::to_string(&input).unwrap_or_default();
                    let tool_call = self
                        .storage
                        .create_tool_call(
                            &self.session_id,
                            &tool_msg.id,
                            &name,
                            &input_str,
                        )
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

                    // Update session to "waiting" for tool approval
                    self.storage
                        .update_session_status(&self.session_id, "waiting")
                        .await?;
                    self.broadcaster.broadcast(
                        "session.statusChanged",
                        json!({ "sessionId": self.session_id, "status": "waiting" }),
                    );

                    // Wait for tool decision via oneshot channel
                    let (tx, rx) = oneshot::channel::<ToolDecision>();
                    self.tool_queue
                        .lock()
                        .unwrap()
                        .insert(tool_call.id.clone(), tx);

                    let decision = rx.await.unwrap_or(ToolDecision::Rejected);

                    // With --dangerously-skip-permissions, claude handles tools itself.
                    // We record the decision and update the status.
                    let (tc_status, output_str) = match decision {
                        ToolDecision::Approved => ("done", "approved".to_string()),
                        ToolDecision::Rejected => ("error", "rejected by user".to_string()),
                    };

                    self.storage
                        .complete_tool_call(&tool_call.id, Some(&output_str), tc_status)
                        .await?;
                    self.broadcaster.broadcast(
                        "session.toolCallUpdated",
                        json!({
                            "sessionId": self.session_id,
                            "toolCallId": tool_call.id,
                            "status": tc_status,
                            "output": output_str
                        }),
                    );

                    // Back to running
                    self.storage
                        .update_session_status(&self.session_id, "running")
                        .await?;
                    self.broadcaster.broadcast(
                        "session.statusChanged",
                        json!({ "sessionId": self.session_id, "status": "running" }),
                    );
                }

                ClaudeEvent::Result { is_error, .. } => {
                    // Finalize streaming message if any
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

                ClaudeEvent::System { .. } | ClaudeEvent::ToolResult { .. } | ClaudeEvent::Unknown => {}
            }
        }

        child.wait().await?;
        Ok(())
    }

    pub async fn resolve_tool(&self, tool_call_id: &str, decision: ToolDecision) -> Result<()> {
        let tx = self
            .tool_queue
            .lock()
            .unwrap()
            .remove(tool_call_id);
        if let Some(tx) = tx {
            let _ = tx.send(decision);
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "tool call not found or already resolved: {}",
                tool_call_id
            ))
        }
    }
}

#[async_trait]
impl Runner for ClaudeCodeRunner {
    async fn send(&self, content: &str) -> Result<()> {
        let tx = self.stdin_tx.read().await;
        if let Some(ref tx) = *tx {
            tx.send(content.to_string())
                .await
                .context("runner stdin channel closed")?;
        } else {
            return Err(anyhow::anyhow!("runner not started"));
        }
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
        *self.stdin_tx.write().await = None;
        Ok(())
    }
}
