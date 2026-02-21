pub mod git;
pub mod watcher;

use crate::ipc::event::EventBroadcaster;
use anyhow::{Context, Result};
use git2::Repository;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tracing::info;

use git::{FileDiff, RepoStatus};

struct RepoEntry {
    path: PathBuf,
    // Keep watcher alive — dropped when entry is removed
    _watcher: notify_debouncer_full::Debouncer<
        notify_debouncer_full::notify::RecommendedWatcher,
        notify_debouncer_full::FileIdMap,
    >,
    last_status: RwLock<RepoStatus>,
}

pub struct RepoRegistry {
    repos: RwLock<HashMap<String, Arc<RepoEntry>>>,
    broadcaster: Arc<EventBroadcaster>,
}

impl RepoRegistry {
    pub fn new(broadcaster: Arc<EventBroadcaster>) -> Self {
        Self {
            repos: RwLock::new(HashMap::new()),
            broadcaster,
        }
    }

    pub fn watched_count(&self) -> usize {
        self.repos.read().unwrap().len()
    }

    /// Register a repo path, start watching it, return its current status.
    pub async fn open(&self, repo_path: &str) -> Result<RepoStatus> {
        let path = PathBuf::from(repo_path);
        let canonical = path.canonicalize().context("path does not exist")?;
        let key = canonical.to_string_lossy().to_string();

        // Already registered?
        {
            let repos = self.repos.read().unwrap();
            if let Some(entry) = repos.get(&key) {
                return Ok(entry.last_status.read().unwrap().clone());
            }
        }

        // Open git repo in background thread (git2 is sync)
        let canonical_clone = canonical.clone();
        let status = tokio::task::spawn_blocking(move || {
            let repo = Repository::open(&canonical_clone)
                .context("not a git repository")?;
            git::read_status(&repo)
        })
        .await??;

        // Start watcher — captures broadcaster + tokio handle for cross-thread spawning.
        // notify fires callbacks on its own OS thread (not a tokio thread), so we must
        // use Handle::current() captured here (inside the async fn) to spawn tasks from it.
        let broadcaster = self.broadcaster.clone();
        let canonical_for_watcher = canonical.clone();
        let rt_handle = tokio::runtime::Handle::current();
        let watcher = watcher::start_watcher(&canonical, move || {
            let canonical_inner = canonical_for_watcher.clone();
            let broadcaster = broadcaster.clone();
            rt_handle.spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    let repo = Repository::open(&canonical_inner)?;
                    git::read_status(&repo)
                })
                .await;
                if let Ok(Ok(new_status)) = result {
                    broadcaster.broadcast(
                        "repo.statusChanged",
                        serde_json::to_value(&new_status).unwrap_or_default(),
                    );
                }
            });
        })?;

        let entry = Arc::new(RepoEntry {
            path: canonical,
            _watcher: watcher,
            last_status: RwLock::new(status.clone()),
        });

        self.repos.write().unwrap().insert(key, entry);
        info!(path = %repo_path, "repo opened");
        Ok(status)
    }

    /// Get current (fresh) status for any repo path.
    pub async fn status(&self, repo_path: &str) -> Result<RepoStatus> {
        let path = PathBuf::from(repo_path)
            .canonicalize()
            .context("path does not exist")?;
        let key = path.to_string_lossy().to_string();

        let status = tokio::task::spawn_blocking(move || {
            let repo = Repository::open(&path).context("repo not found")?;
            git::read_status(&repo)
        })
        .await??;

        // Update cached entry if tracked
        if let Some(entry) = self.repos.read().unwrap().get(&key) {
            *entry.last_status.write().unwrap() = status.clone();
        }

        Ok(status)
    }

    pub async fn diff(&self, repo_path: &str) -> Result<Vec<FileDiff>> {
        let path = PathBuf::from(repo_path)
            .canonicalize()
            .context("path does not exist")?;
        tokio::task::spawn_blocking(move || {
            let repo = Repository::open(&path).context("repo not found")?;
            git::read_diff(&repo)
        })
        .await?
    }

    pub async fn file_diff(
        &self,
        repo_path: &str,
        file_path: &str,
        staged: bool,
    ) -> Result<FileDiff> {
        let path = PathBuf::from(repo_path)
            .canonicalize()
            .context("path does not exist")?;
        let file_path = file_path.to_string();
        tokio::task::spawn_blocking(move || {
            let repo = Repository::open(&path).context("repo not found")?;
            git::read_file_diff(&repo, &file_path, staged)
        })
        .await?
    }
}
