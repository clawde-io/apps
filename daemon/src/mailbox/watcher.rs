// SPDX-License-Identifier: MIT
// Sprint N — Mailbox filesystem watcher (MR.T06, MR.T09).
//
// Watches `{repo}/.claude/inbox/` directories for new `.md` files.
// On detection:
//   1. Parse YAML front-matter → MailboxMessage
//   2. Persist to SQLite via MailboxStorage.insert_with_id()
//   3. Emit `inbox.messageReceived` push event via EventBroadcaster

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify::event::CreateKind;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::ipc::event::EventBroadcaster;
use crate::mailbox::model::MailboxMessage;
use crate::mailbox::storage::MailboxStorage;

// ─── MailboxWatcher ────────────────────────────────────────────────────────────

/// Watches one or more `.claude/inbox/` directories and ingests new messages
/// into the daemon's SQLite database, then broadcasts a push event to all
/// connected clients.
pub struct MailboxWatcher {
    broadcaster: Arc<EventBroadcaster>,
    storage:     MailboxStorage,
    /// Watched inbox directories.
    watched:     Vec<PathBuf>,
}

impl MailboxWatcher {
    pub fn new(storage: MailboxStorage, broadcaster: Arc<EventBroadcaster>) -> Self {
        Self {
            broadcaster,
            storage,
            watched: Vec::new(),
        }
    }

    /// Register a repo's `.claude/inbox/` directory for watching.
    ///
    /// Idempotent — adding the same path twice is harmless.
    pub fn add_repo(&mut self, repo_path: &str) {
        let inbox = Path::new(repo_path).join(".claude/inbox");
        if !self.watched.iter().any(|w| w == &inbox) {
            self.watched.push(inbox);
        }
    }

    /// Start the watcher on a dedicated Tokio background task.
    ///
    /// This method consumes `self` and spawns a long-running task; call it
    /// once at daemon startup after all repos have been registered.
    pub fn run(self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<Result<Event, notify::Error>>(64);

        // notify callbacks run on a notify-internal thread.  We forward events
        // to a tokio mpsc channel and process them on the async executor.
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.blocking_send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        for inbox_dir in &self.watched {
            if let Err(e) = std::fs::create_dir_all(inbox_dir) {
                warn!(path = %inbox_dir.display(), err = %e, "could not create inbox dir — skipping watch");
                continue;
            }
            if let Err(e) = watcher.watch(inbox_dir, RecursiveMode::NonRecursive) {
                warn!(path = %inbox_dir.display(), err = %e, "could not watch inbox dir");
            } else {
                info!(path = %inbox_dir.display(), "watching mailbox inbox");
            }
        }

        let broadcaster = Arc::clone(&self.broadcaster);
        let storage     = self.storage.clone();

        tokio::spawn(async move {
            // Keep the watcher alive for the duration of the spawned task.
            let _watcher = watcher;

            while let Some(event_res) = rx.recv().await {
                match event_res {
                    Ok(event) => {
                        if let EventKind::Create(CreateKind::File) = event.kind {
                            for path in event.paths {
                                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                                    handle_new_inbox_file(&path, &storage, &broadcaster).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(err = %e, "mailbox watcher error");
                    }
                }
            }
        });

        Ok(())
    }
}

// ─── File ingestion ───────────────────────────────────────────────────────────

async fn handle_new_inbox_file(
    path:        &Path,
    storage:     &MailboxStorage,
    broadcaster: &EventBroadcaster,
) {
    // Derive the file ID from the filename stem (minus `.md`).
    let file_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Read the file with a small retry loop — the watcher fires as soon as the
    // rename from `.tmp` to `.md` completes, but the file might still be
    // flushed by the OS.
    let content = match read_with_retry(path, 3).await {
        Ok(c)  => c,
        Err(e) => {
            warn!(path = %path.display(), err = %e, "failed to read inbox file");
            return;
        }
    };

    let msg = match MailboxMessage::from_markdown(&content, &file_id) {
        Some(m) => m,
        None => {
            warn!(path = %path.display(), "inbox file has no valid front-matter — skipping");
            return;
        }
    };

    debug!(id = %msg.id, from = %msg.from_repo, subject = %msg.subject, "inbox message received");

    if let Err(e) = storage.insert_with_id(&msg).await {
        warn!(id = %msg.id, err = %e, "failed to persist inbox message");
        return;
    }

    broadcaster.broadcast(
        "inbox.messageReceived",
        serde_json::json!({
            "id":       msg.id,
            "fromRepo": msg.from_repo,
            "toRepo":   msg.to_repo,
            "subject":  msg.subject,
        }),
    );
}

async fn read_with_retry(path: &Path, attempts: u32) -> Result<String> {
    let mut last_err = anyhow::anyhow!("unreachable");
    for i in 0..attempts {
        if i > 0 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        match tokio::fs::read_to_string(path).await {
            Ok(c) => return Ok(c),
            Err(e) => last_err = e.into(),
        }
    }
    Err(last_err)
}
