use super::events::EventLog;
use super::runner::Runner;
use crate::{
    account::{AccountRegistry, PickHint},
    ipc::event::EventBroadcaster,
    license::LicenseInfo,
    storage::Storage,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{Mutex, RwLock},
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
    /// PID of the currently running `claude` subprocess (0 = no child).
    /// Used on Unix to send SIGSTOP / SIGCONT for real pause / resume.
    child_pid: Arc<AtomicU32>,
    /// The currently running `claude` subprocess, if any.
    /// Shared between run_turn (which stores and waits) and stop (which kills).
    current_child: Arc<Mutex<Option<Child>>>,
    /// Set to true by stop() so the event_loop safety net does not overwrite
    /// the "idle" status broadcast by SessionManager::cancel().
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    /// Multi-account pool — used to pick the best account before each turn
    /// and to detect + record rate-limit events from stderr output.
    account_registry: Arc<AccountRegistry>,
    /// Current license tier — needed by mark_limited() to decide between
    /// auto-switch (Personal Remote+) and manual-prompt (Free) behaviour.
    license: Arc<tokio::sync::RwLock<LicenseInfo>>,
}

impl ClaudeCodeRunner {
    pub fn new(
        session_id: String,
        repo_path: String,
        storage: Arc<Storage>,
        data_dir: std::path::PathBuf,
        broadcaster: Arc<EventBroadcaster>,
        account_registry: Arc<AccountRegistry>,
        license: Arc<tokio::sync::RwLock<LicenseInfo>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            event_log: EventLog::new(&data_dir, &session_id),
            session_id,
            repo_path,
            storage,
            broadcaster,
            claude_session_id: RwLock::new(None),
            paused: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            child_pid: Arc::new(AtomicU32::new(0)),
            current_child: Arc::new(Mutex::new(None)),
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            account_registry,
            license,
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

        // Pick the best available account for this turn.
        // On rate-limit detection (via stderr) we'll call mark_limited() on it.
        let hint = PickHint {
            provider: Some("claude".to_string()),
        };
        let picked_account = self.account_registry.pick_account(&hint).await?;

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

        // Apply per-account credentials if configured.
        // The `credentials_path` is an alternative config directory that the
        // claude CLI will use instead of the default ~/.claude.  This allows
        // multiple accounts to coexist on the same machine.
        if let Some(ref account) = picked_account {
            if !account.credentials_path.is_empty() {
                cmd.env("CLAUDE_CONFIG_DIR", &account.credentials_path);
            }
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

        // Drain stderr and detect rate-limit signals in real time.
        // On detection, mark the account limited and broadcast the appropriate
        // event (auto-switch for Personal Remote+, manual prompt for Free).
        let picked_account_id = picked_account.map(|a| a.id);
        let account_registry = self.account_registry.clone();
        let license = self.license.clone();
        let session_id_for_limit = self.session_id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!(target: "claude_stderr", "{}", line);
                if let Some(cooldown) = AccountRegistry::detect_limit_signal(&line) {
                    if let Some(ref acct_id) = picked_account_id {
                        let lic = license.read().await;
                        let _ = account_registry
                            .mark_limited(acct_id, &session_id_for_limit, cooldown, &lic)
                            .await;
                    }
                }
            }
        });

        // Store the PID before handing ownership of the Child to the mutex.
        // Used by pause() / resume() to send SIGSTOP / SIGCONT on Unix.
        if let Some(pid) = child.id() {
            self.child_pid.store(pid, Ordering::Relaxed);
        }

        // Store the child handle so stop() can kill it if needed.
        *self.current_child.lock().await = Some(child);

        self.event_loop(stdout).await
    }

    async fn event_loop(&self, stdout: tokio::process::ChildStdout) -> Result<()> {
        let mut lines = BufReader::new(stdout).lines();
        let mut current_message_id: Option<String> = None;
        let mut current_content = String::new();
        let mut received_result = false;

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
                    received_result = true;
                }

                ClaudeEvent::ToolResult { .. } | ClaudeEvent::Unknown => {}
            }
        }

        // Reap the child process if stop() hasn't already done so.
        if let Some(mut child) = self.current_child.lock().await.take() {
            let _ = child.wait().await;
        }
        // Child is gone — clear the stored PID so pause/resume are no-ops.
        self.child_pid.store(0, Ordering::Relaxed);

        // Safety net: if the process exited without emitting a Result event
        // (e.g. crashed or was killed externally), mark the session as error.
        // Skip this if stop() was called intentionally — SessionManager::cancel()
        // will set the status to "idle" after stop() returns.
        if !received_result && !self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            let _ = self
                .storage
                .update_session_status(&self.session_id, "error")
                .await;
            self.broadcaster.broadcast(
                "session.statusChanged",
                json!({ "sessionId": self.session_id, "status": "error" }),
            );
        }

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
        // On Unix, send SIGSTOP to actually suspend the subprocess.
        // On other platforms the flag is set but the process keeps running
        // (no native suspend mechanism available without platform-specific code).
        #[cfg(unix)]
        {
            let pid = self.child_pid.load(Ordering::Relaxed);
            if pid != 0 {
                // SAFETY: pid is a valid positive process ID obtained from the
                // spawned child.  SIGSTOP is safe to send to our own child.
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGSTOP);
                }
            }
        }
        Ok(())
    }

    async fn resume(&self) -> Result<()> {
        self.paused
            .store(false, std::sync::atomic::Ordering::Relaxed);
        // Send SIGCONT to wake the subprocess back up.
        #[cfg(unix)]
        {
            let pid = self.child_pid.load(Ordering::Relaxed);
            if pid != 0 {
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGCONT);
                }
            }
        }
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        // Mark as intentionally cancelled before killing so the event_loop
        // safety net does not race and mark the session as "error".
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        // If the process is paused, send SIGCONT first so SIGKILL is delivered.
        #[cfg(unix)]
        {
            let pid = self.child_pid.load(Ordering::Relaxed);
            if pid != 0 && self.paused.load(std::sync::atomic::Ordering::Relaxed) {
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGCONT);
                }
            }
        }
        if let Some(mut child) = self.current_child.lock().await.take() {
            // Send SIGKILL (on Unix) / TerminateProcess (on Windows).
            // Ignore errors — the process may have already exited.
            let _ = child.kill().await;
            // Reap the process so we don't leave a zombie.
            let _ = child.wait().await;
        }
        self.child_pid.store(0, Ordering::Relaxed);
        Ok(())
    }
}
