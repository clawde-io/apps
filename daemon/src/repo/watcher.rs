use anyhow::Result;
use notify_debouncer_full::{
    new_debouncer,
    notify::{RecursiveMode, Watcher},
    DebounceEventResult,
};
use std::{path::Path, time::Duration};
use tracing::warn;

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
