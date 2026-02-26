use anyhow::Result;
use notify_debouncer_full::{
    new_debouncer,
    notify::{RecursiveMode, Watcher},
    DebounceEventResult,
};
use std::{path::Path, sync::Arc, time::Duration};
use tracing::warn;

// ─── Basic watcher (legacy) ───────────────────────────────────────────────────

/// Starts a debounced file watcher on `repo_path`.
/// When changes are detected, triggers `on_change`.
pub fn start_watcher<F>(
    repo_path: &Path,
    on_change: F,
) -> Result<
    notify_debouncer_full::Debouncer<
        notify_debouncer_full::notify::RecommendedWatcher,
        notify_debouncer_full::FileIdMap,
    >,
>
where
    F: Fn() + Send + 'static,
{
    let mut debouncer = new_debouncer(
        Duration::from_millis(300),
        None,
        move |result: DebounceEventResult| match result {
            Ok(_events) => on_change(),
            Err(errors) => {
                for e in errors {
                    warn!(err = %e, "file watcher error");
                }
            }
        },
    )?;

    debouncer
        .watcher()
        .watch(repo_path, RecursiveMode::Recursive)?;

    Ok(debouncer)
}

// ─── Ambient watch mode (Sprint BB PV.17) ────────────────────────────────────

/// Suggestion trigger: why a `watch.suggestion` event was fired.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchSuggestion {
    /// The file that triggered the suggestion.
    pub file: String,
    /// Human-readable reason for the suggestion.
    pub reason: WatchSuggestionReason,
    /// The matched TODO text (if `reason == TodoComment`).
    pub todo_text: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WatchSuggestionReason {
    /// File was saved containing a TODO/FIXME comment.
    TodoComment,
    /// File was modified and matches an active task's target path.
    ActiveTaskMatch,
    /// Drift detected: file content diverges from the tracked spec.
    DriftDetected,
}

/// Inspect a file that was just saved and return a suggestion if warranted.
///
/// Checks:
/// 1. File contains a `TODO` or `FIXME` comment.
/// 2. Filename matches any active task path pattern.
///
/// Returns `None` if no actionable signal is found.
pub fn inspect_file_for_suggestions(
    file_path: &Path,
    active_task_paths: &[String],
) -> Option<WatchSuggestion> {
    let file_str = file_path.to_string_lossy().to_string();

    // Skip non-source files (binaries, lock files, build artefacts).
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !matches!(ext, "rs" | "dart" | "ts" | "tsx" | "js" | "py" | "go" | "md" | "toml") {
        return None;
    }

    // Check for TODO/FIXME in the file (read synchronously — called from the
    // watcher callback which already runs on a blocking thread).
    if let Ok(content) = std::fs::read_to_string(file_path) {
        for line in content.lines() {
            let lower = line.to_ascii_lowercase();
            if lower.contains("todo") || lower.contains("fixme") {
                let todo_text = line.trim().chars().take(120).collect::<String>();
                return Some(WatchSuggestion {
                    file: file_str.clone(),
                    reason: WatchSuggestionReason::TodoComment,
                    todo_text: Some(todo_text),
                });
            }
        }
    }

    // Check if the file matches any active task's target path.
    for task_path in active_task_paths {
        if file_str.contains(task_path.as_str()) {
            return Some(WatchSuggestion {
                file: file_str.clone(),
                reason: WatchSuggestionReason::ActiveTaskMatch,
                todo_text: None,
            });
        }
    }

    None
}

/// Start an ambient watcher that fires `watch.suggestion` push events via the
/// provided broadcaster when actionable signals are detected in the repo.
///
/// This is the entry point for Sprint BB PV.17. It wraps `start_watcher` and
/// adds the suggestion-detection logic on top.
pub fn start_ambient_watcher(
    repo_path: &Path,
    broadcaster: Arc<crate::ipc::event::EventBroadcaster>,
    repo_path_str: String,
) -> Result<
    notify_debouncer_full::Debouncer<
        notify_debouncer_full::notify::RecommendedWatcher,
        notify_debouncer_full::FileIdMap,
    >,
> {
    let mut debouncer = new_debouncer(
        Duration::from_millis(500), // slightly longer debounce for ambient mode
        None,
        move |result: DebounceEventResult| {
            let events = match result {
                Ok(e) => e,
                Err(errors) => {
                    for e in errors {
                        warn!(err = %e, "ambient watcher error");
                    }
                    return;
                }
            };

            // Collect paths of files that were modified/created.
            let changed: Vec<std::path::PathBuf> = events
                .iter()
                .flat_map(|e| e.paths.iter().cloned())
                .filter(|p| p.is_file())
                .collect();

            for path in changed {
                // For now pass empty task paths — integration with live task
                // storage happens via a future RPC (task.watch_paths).
                if let Some(suggestion) = inspect_file_for_suggestions(&path, &[]) {
                    broadcaster.broadcast(
                        "watch.suggestion",
                        serde_json::json!({
                            "repoPath": repo_path_str,
                            "file": suggestion.file,
                            "reason": suggestion.reason,
                            "todoText": suggestion.todo_text,
                        }),
                    );
                }
            }
        },
    )?;

    debouncer
        .watcher()
        .watch(repo_path, RecursiveMode::Recursive)?;

    Ok(debouncer)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::with_suffix(".rs").unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn detects_todo_comment() {
        let f = write_temp("fn foo() {\n    // TODO: implement this\n    todo!()\n}\n");
        let result = inspect_file_for_suggestions(f.path(), &[]);
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap().reason,
            WatchSuggestionReason::TodoComment
        ));
    }

    #[test]
    fn detects_fixme_comment() {
        let f = write_temp("// FIXME: broken\nfn bar() {}\n");
        let result = inspect_file_for_suggestions(f.path(), &[]);
        assert!(result.is_some());
    }

    #[test]
    fn no_suggestion_for_clean_file() {
        let f = write_temp("fn clean() -> bool { true }\n");
        let result = inspect_file_for_suggestions(f.path(), &[]);
        assert!(result.is_none());
    }

    #[test]
    fn detects_active_task_path_match() {
        let f = write_temp("fn ok() {}\n");
        let path_str = f.path().to_string_lossy().to_string();
        // Pass a fragment of the path as an "active task path".
        let result = inspect_file_for_suggestions(f.path(), &[path_str.clone()]);
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap().reason,
            WatchSuggestionReason::ActiveTaskMatch
        ));
    }

    #[test]
    fn skips_lock_file() {
        let mut f = NamedTempFile::with_suffix(".lock").unwrap();
        f.write_all(b"some lock content // TODO: should be skipped").unwrap();
        let result = inspect_file_for_suggestions(f.path(), &[]);
        assert!(result.is_none());
    }
}
