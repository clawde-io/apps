//! AfsWatcher — watches `.claude/` directories for human edits and file system events.
//! Uses the `notify` crate (already in Cargo.toml) with 200ms debounce.

use super::{markdown_generator, markdown_parser, queue_serializer, storage::TaskStorage};
use crate::ipc::event::EventBroadcaster;
use anyhow::Result;
// Use notify through notify_debouncer_full to avoid version conflicts
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode, Watcher},
    DebounceEventResult,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Ignore patterns — never process these paths.
fn should_ignore(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Daemon's own output — don't re-process
    if path_str.ends_with("queue.json") || path_str.ends_with("queue.json.tmp") {
        return true;
    }

    for segment in path.components() {
        let s = segment.as_os_str().to_string_lossy();
        if matches!(
            s.as_ref(),
            "node_modules" | ".git" | "target" | "build" | ".next" | "dist"
        ) {
            return true;
        }
    }

    // Temp directory
    if path_str.contains("/.claude/temp/") {
        return true;
    }

    // OS/editor noise
    if let Some(name) = path.file_name() {
        let n = name.to_string_lossy();
        if n.starts_with(".DS_Store")
            || n.ends_with(".swp")
            || n.ends_with(".swo")
            || n.ends_with('~')
            || n.ends_with(".tmp")
            || n.ends_with(".temp")
        {
            return true;
        }
    }

    false
}

/// Classify a path change within a project.
#[derive(Debug)]
enum AfsEvent {
    ActiveMd,
    PlanningUpdated(PathBuf),
    QaItemChecked(PathBuf),
    InboxMessage(PathBuf),
    MemoryUpdated(PathBuf),
    HumanFileEdit(PathBuf),
}

fn classify_path(path: &Path, project_root: &Path) -> Option<AfsEvent> {
    let rel = path.strip_prefix(project_root).ok()?;
    let rel_str = rel.to_string_lossy();

    if rel_str == ".claude/tasks/active.md" {
        return Some(AfsEvent::ActiveMd);
    }
    if rel_str.starts_with(".claude/planning/") {
        return Some(AfsEvent::PlanningUpdated(path.to_path_buf()));
    }
    if rel_str.starts_with(".claude/qa/") {
        return Some(AfsEvent::QaItemChecked(path.to_path_buf()));
    }
    if rel_str.starts_with(".claude/inbox/") && path.extension().map(|e| e == "md").unwrap_or(false)
    {
        return Some(AfsEvent::InboxMessage(path.to_path_buf()));
    }
    if rel_str.starts_with(".claude/memory/") {
        return Some(AfsEvent::MemoryUpdated(path.to_path_buf()));
    }
    // Non-.claude source files (human edits)
    if !rel_str.starts_with(".claude/") && !rel_str.starts_with('.') {
        return Some(AfsEvent::HumanFileEdit(path.to_path_buf()));
    }

    None
}

pub struct AfsWatcher {
    storage: Arc<TaskStorage>,
    broadcaster: Arc<EventBroadcaster>,
    // Map: project root -> last active.md hash (to avoid feedback loops)
    active_md_hashes: Arc<Mutex<HashMap<PathBuf, u128>>>,
}

impl AfsWatcher {
    pub fn new(storage: Arc<TaskStorage>, broadcaster: Arc<EventBroadcaster>) -> Self {
        Self {
            storage,
            broadcaster,
            active_md_hashes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start watching a project directory. Runs on a background thread.
    pub fn watch_project(self: Arc<Self>, project_root: PathBuf) -> Result<()> {
        let storage = self.storage.clone();
        let broadcaster = self.broadcaster.clone();
        let hashes = self.active_md_hashes.clone();
        let root = project_root.clone();

        // Spawn a blocking thread for the file watcher (notify uses sync callbacks)
        std::thread::spawn(move || {
            let rt = tokio::runtime::Handle::current();

            let storage_c = storage.clone();
            let broadcaster_c = broadcaster.clone();
            let hashes_c = hashes.clone();
            let root_c = root.clone();

            let handler = move |result: DebounceEventResult| {
                // Wrap in catch_unwind so a panicking path handler doesn't
                // kill the watcher thread (and silently stop all AFS events).
                let guard_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    match result {
                        Ok(events) => {
                            for event in events {
                                let path = match event.event.paths.first() {
                                    Some(p) => p.clone(),
                                    None => continue,
                                };

                                if should_ignore(&path) {
                                    continue;
                                }

                                // Only process write/create events
                                let is_write = matches!(
                                    event.event.kind,
                                    EventKind::Modify(_) | EventKind::Create(_)
                                );
                                if !is_write {
                                    continue;
                                }

                                if let Some(afs_event) = classify_path(&path, &root_c) {
                                    let storage_i = storage_c.clone();
                                    let broadcaster_i = broadcaster_c.clone();
                                    let hashes_i = hashes_c.clone();
                                    let root_i = root_c.clone();

                                    rt.spawn(async move {
                                        if let Err(e) = handle_afs_event(
                                            afs_event,
                                            &root_i,
                                            &storage_i,
                                            &broadcaster_i,
                                            &hashes_i,
                                        )
                                        .await
                                        {
                                            warn!("AFS event handling error: {e}");
                                        }
                                    });
                                }
                            }
                        }
                        Err(errors) => {
                            for e in errors {
                                warn!("Watcher error: {e:?}");
                            }
                        }
                    }
                }));
                if let Err(panic_val) = guard_result {
                    warn!("AFS watcher handler panicked: {:?}", panic_val);
                }
            };

            let mut debouncer = match new_debouncer(Duration::from_millis(200), None, handler) {
                Ok(d) => d,
                Err(e) => {
                    warn!("Failed to create file watcher: {e}");
                    return;
                }
            };

            if let Err(e) = debouncer.watcher().watch(&root, RecursiveMode::Recursive) {
                warn!("Failed to watch {}: {e}", root.display());
                return;
            }

            info!("AFS watcher active for {}", root.display());

            // Keep thread alive — debouncer will drop on thread exit
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        });

        Ok(())
    }
}

async fn handle_afs_event(
    event: AfsEvent,
    project_root: &Path,
    storage: &TaskStorage,
    broadcaster: &EventBroadcaster,
    hashes: &Mutex<HashMap<PathBuf, u128>>,
) -> Result<()> {
    let repo_path = project_root.to_string_lossy().to_string();

    match event {
        AfsEvent::ActiveMd => {
            let active_md_path = project_root.join(".claude/tasks/active.md");
            let content = match tokio::fs::read_to_string(&active_md_path).await {
                Ok(c) => c,
                Err(_) => return Ok(()),
            };

            // Change detection: skip if hash unchanged (prevents feedback loop)
            let new_hash = content_hash(&content);
            {
                let mut h = hashes.lock().await;
                let prev = h.entry(project_root.to_path_buf()).or_insert(0);
                if *prev == new_hash {
                    return Ok(());
                }
                *prev = new_hash;
            }

            debug!("active.md changed, syncing to DB");

            let parsed = markdown_parser::parse_active_md(&content);
            let count = storage.backfill_from_tasks(parsed, &repo_path).await?;
            if count > 0 {
                info!("AFS sync: imported {count} new tasks from active.md");
            }

            // Regenerate queue.json
            queue_serializer::flush_queue(storage, &repo_path).await?;

            // Regenerate active.md from DB to sync status symbols back to the file.
            // Re-read DB tasks and write the updated markdown to avoid stale symbols.
            let db_tasks = storage
                .list_tasks(&super::storage::TaskListParams {
                    repo_path: Some(repo_path.clone()),
                    ..Default::default()
                })
                .await?;
            let updated_md = markdown_generator::regenerate(&content, &db_tasks);
            if updated_md != content {
                // Update hash so the watcher ignores the write-back we're about to do.
                let updated_hash = content_hash(&updated_md);
                {
                    let mut h = hashes.lock().await;
                    h.insert(project_root.to_path_buf(), updated_hash);
                }
                tokio::fs::write(&active_md_path, &updated_md).await?;
                debug!("AFS: wrote updated status symbols back to active.md");
            }

            broadcaster.broadcast(
                "afs.activeMdSynced",
                serde_json::json!({ "repo_path": repo_path, "imported": count }),
            );
        }

        AfsEvent::PlanningUpdated(path) => {
            let rel = path
                .strip_prefix(project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            storage
                .log_activity(
                    "system",
                    None,
                    None,
                    "planning_updated",
                    "auto",
                    Some(&rel),
                    None,
                    &repo_path,
                )
                .await?;
            broadcaster.broadcast(
                "afs.planningUpdated",
                serde_json::json!({ "repo_path": repo_path, "file": rel }),
            );
        }

        AfsEvent::QaItemChecked(path) => {
            let rel = path
                .strip_prefix(project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            storage
                .log_activity(
                    "system",
                    None,
                    None,
                    "qa_item_checked",
                    "auto",
                    Some(&rel),
                    None,
                    &repo_path,
                )
                .await?;
            broadcaster.broadcast(
                "afs.qaItemChecked",
                serde_json::json!({ "repo_path": repo_path, "file": rel }),
            );
        }

        AfsEvent::InboxMessage(path) => {
            let rel = path
                .strip_prefix(project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            broadcaster.broadcast(
                "inbox.messageReceived",
                serde_json::json!({ "repo_path": repo_path, "file": rel }),
            );
        }

        AfsEvent::MemoryUpdated(path) => {
            let rel = path
                .strip_prefix(project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            storage
                .log_activity(
                    "system",
                    None,
                    None,
                    "memory_updated",
                    "auto",
                    Some(&rel),
                    None,
                    &repo_path,
                )
                .await?;
        }

        AfsEvent::HumanFileEdit(path) => {
            let rel = path
                .strip_prefix(project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            // Attribute to human via git config name (best-effort)
            let agent = "human:unknown";
            storage
                .log_activity(
                    agent,
                    None,
                    None,
                    "human_file_edit",
                    "auto",
                    Some(&rel),
                    None,
                    &repo_path,
                )
                .await?;
        }
    }

    Ok(())
}

/// SHA-256 based content hash for change detection.
/// Returns the first 16 bytes as u128 for compact storage in the hash map
/// while providing 128 bits of collision resistance.
fn content_hash(s: &str) -> u128 {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(s.as_bytes());
    u128::from_le_bytes(hash[..16].try_into().expect("SHA-256 produces 32 bytes"))
}
