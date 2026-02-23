/// Background jobs for the task system.
/// All jobs run on tokio intervals — started from AppContext in lib.rs.

use super::storage::TaskStorage;
use crate::ipc::event::EventBroadcaster;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

/// Heartbeat checker: runs every 30s.
/// Tasks in_progress with last_heartbeat > 90s ago → interrupted.
pub async fn run_heartbeat_checker(
    storage: Arc<TaskStorage>,
    broadcaster: Arc<EventBroadcaster>,
    timeout_secs: i64,
) {
    let mut ticker = interval(Duration::from_secs(30));
    loop {
        ticker.tick().await;

        match storage.interrupt_stale_tasks(timeout_secs).await {
            Ok(interrupted_ids) => {
                for id in &interrupted_ids {
                    info!("Task {id} interrupted (heartbeat timeout)");

                    // Log system activity
                    let reason = format!("heartbeat timeout after {}s", timeout_secs);
                    let _ = storage
                        .log_activity(
                            "system",
                            Some(id),
                            None,
                            "task_released",
                            "system",
                            Some(&reason),
                            None,
                            "",
                        )
                        .await;

                    broadcaster.broadcast(
                        "task.interrupted",
                        serde_json::json!({
                            "task_id": id,
                            "reason": reason
                        }),
                    );
                }
            }
            Err(e) => warn!("Heartbeat checker error: {e}"),
        }
    }
}

/// Done task archiver: runs every hour.
/// Moves done tasks older than `visible_hours` to agent_tasks_archive.
/// NEVER archives interrupted tasks.
pub async fn run_done_task_archiver(
    storage: Arc<TaskStorage>,
    visible_hours: i64,
) {
    let mut ticker = interval(Duration::from_secs(3600));
    loop {
        ticker.tick().await;

        match storage.archive_done_tasks(visible_hours).await {
            Ok(count) if count > 0 => info!("Archived {count} done tasks"),
            Ok(_) => {}
            Err(e) => warn!("Done task archiver error: {e}"),
        }
    }
}

/// Activity log pruner: runs daily at approx 4am local time (simplified to every 24h).
pub async fn run_activity_log_pruner(storage: Arc<TaskStorage>, retention_days: i64) {
    let mut ticker = interval(Duration::from_secs(86400));
    loop {
        ticker.tick().await;

        match storage.prune_activity_log(retention_days).await {
            Ok(count) if count > 0 => info!("Pruned {count} old activity log entries"),
            Ok(_) => {}
            Err(e) => warn!("Activity log pruner error: {e}"),
        }
    }
}

/// Work session tracker: closes open work session every 4h and opens a new one.
pub async fn run_work_session_tracker(storage: Arc<TaskStorage>, repo_path: String) {
    // Open initial session
    let mut current_session_id = match storage.open_work_session(&repo_path).await {
        Ok(s) => {
            info!("Work session opened: {}", s.id);
            s.id
        }
        Err(e) => {
            warn!("Failed to open work session: {e}");
            return;
        }
    };

    let mut ticker = interval(Duration::from_secs(4 * 3600));
    ticker.tick().await; // skip initial tick (already opened)

    loop {
        ticker.tick().await;

        // Close current session
        if let Err(e) = storage.close_work_session(&current_session_id, 0, 0).await {
            warn!("Failed to close work session: {e}");
        }

        // Open new session
        match storage.open_work_session(&repo_path).await {
            Ok(s) => {
                current_session_id = s.id;
            }
            Err(e) => warn!("Failed to open new work session: {e}"),
        }
    }
}
