/// Background drift scanner — runs every 24 h and emits push events.
///
/// V02.T25: Triggered on session.create AND on a 24h interval.
/// Emits `session.driftDetected` when new drift items are found.
use crate::drift::{scanner, storage};
use crate::ipc::event::EventBroadcaster;
use crate::storage::Storage;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const SCAN_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Spawn the 24h background drift scanner.
pub fn spawn(storage: Arc<Storage>, broadcaster: Arc<EventBroadcaster>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(SCAN_INTERVAL);
        interval.tick().await; // skip immediate first tick — don't scan on startup
        loop {
            interval.tick().await;
            if let Err(e) = run_scan_all(&storage, &broadcaster).await {
                warn!(error = %e, "background drift scan failed");
            }
        }
    });
}

/// Run a drift scan for all registered projects (or just the daemon data dir).
async fn run_scan_all(storage: &Arc<Storage>, broadcaster: &Arc<EventBroadcaster>) -> anyhow::Result<()> {
    // Scan the daemon's own source tree as a primary target.
    // In a real deployment the daemon knows all registered project paths via
    // the projects table; here we use a best-effort scan of the data dir.
    let projects = list_registered_projects(storage).await;

    for project_path in projects {
        scan_project_and_notify(storage, broadcaster, &project_path).await;
    }
    Ok(())
}

/// Return all registered project paths (from projects table if available).
async fn list_registered_projects(storage: &Arc<Storage>) -> Vec<std::path::PathBuf> {
    // Query the projects table — if it doesn't exist yet return empty.
    let pool = storage.pool();
    let rows: Vec<(String,)> = sqlx::query_as("SELECT path FROM projects LIMIT 50")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    rows.into_iter()
        .map(|(p,)| std::path::PathBuf::from(p))
        .collect()
}

/// Scan a single project and push a notification if new items were found.
pub async fn scan_project_and_notify(
    storage: &Arc<Storage>,
    broadcaster: &Arc<EventBroadcaster>,
    project_path: &Path,
) {
    let project_str = project_path.to_string_lossy().to_string();
    debug!(project = %project_str, "drift scan starting");

    // Count items before scan to detect new ones.
    let before = storage::count_unresolved(&storage.pool(), &project_str)
        .await
        .unwrap_or(0);

    match scanner::scan(project_path).await {
        Err(e) => warn!(project = %project_str, error = %e, "drift scan error"),
        Ok(items) => {
            let count = items.len();
            if let Err(e) = storage::clear_unresolved(&storage.pool(), &project_str).await {
                warn!(error = %e, "failed to clear old drift items");
                return;
            }
            if !items.is_empty() {
                if let Err(e) = storage::upsert_items(&storage.pool(), &items).await {
                    warn!(error = %e, "failed to store drift items");
                    return;
                }
            }

            let after = storage::count_unresolved(&storage.pool(), &project_str)
                .await
                .unwrap_or(0);

            info!(project = %project_str, before, after, new = count, "drift scan complete");

            // Emit push event only when items were found or count changed.
            if after > 0 || (before > 0 && after == 0) {
                broadcaster.broadcast(
                    "session.driftDetected",
                    serde_json::json!({
                        "project_path": project_str,
                        "count": after,
                        "new_items": count,
                    }),
                );
            }
        }
    }
}
