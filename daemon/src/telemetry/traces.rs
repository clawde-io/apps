//! JSONL trace storage — append-only, with file rotation at 50 MB.
//!
//! Trace records are written one JSON object per line to
//! `.claw/telemetry/traces.jsonl`.  When the active file exceeds 50 MB it is
//! renamed to `traces-{timestamp}.jsonl` and a fresh `traces.jsonl` is opened.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::warn;

use super::schema::TraceEvent;

/// Maximum active trace file size before rotation.
const MAX_TRACE_FILE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB

// ─── TracesWriter ─────────────────────────────────────────────────────────────

/// Async writer for the append-only JSONL trace log.
///
/// Thread-safe: wraps the file handle and byte counter behind an `Arc<Mutex<_>>`
/// so multiple emitters can share a single writer.
pub struct TracesWriter {
    /// Directory that holds `traces.jsonl` and rotated files.
    telemetry_dir: PathBuf,
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    file: tokio::fs::File,
    bytes_written: u64,
}

impl TracesWriter {
    /// Open (or create) the active trace file.  The parent directory must already exist.
    pub async fn new(data_dir: &Path) -> Result<Self> {
        let telemetry_dir = data_dir.join("telemetry");
        tokio::fs::create_dir_all(&telemetry_dir)
            .await
            .with_context(|| format!("create telemetry dir: {}", telemetry_dir.display()))?;

        let file_path = telemetry_dir.join("traces.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await
            .with_context(|| format!("open traces file: {}", file_path.display()))?;

        let metadata = file
            .metadata()
            .await
            .with_context(|| "read traces file metadata")?;
        let bytes_written = metadata.len();

        Ok(Self {
            telemetry_dir,
            inner: Arc::new(Mutex::new(Inner {
                file,
                bytes_written,
            })),
        })
    }

    /// Append a single trace event as a JSON line.
    ///
    /// Rotates the file first if the current size exceeds `MAX_TRACE_FILE_BYTES`.
    pub async fn write(&self, event: &TraceEvent) -> Result<()> {
        let mut inner = self.inner.lock().await;

        // Rotate if needed before writing.
        if inner.bytes_written >= MAX_TRACE_FILE_BYTES {
            if let Err(e) = self.rotate(&mut inner).await {
                warn!(err = %e, "telemetry: trace file rotation failed — continuing on current file");
            }
        }

        let mut line = serde_json::to_string(event).context("serialize trace event")?;
        line.push('\n');
        let bytes = line.as_bytes();

        inner
            .file
            .write_all(bytes)
            .await
            .context("write trace event")?;
        inner.bytes_written += bytes.len() as u64;

        Ok(())
    }

    /// Read and filter trace events from the active file.
    ///
    /// Filtering is done in-process (no index); suitable for low-frequency
    /// debug/audit queries.  For high-volume use, build an index separately.
    pub async fn query(
        &self,
        task_id: Option<&str>,
        since: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<TraceEvent>> {
        let file_path = self.telemetry_dir.join("traces.jsonl");
        let content = match tokio::fs::read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).context("read traces file"),
        };

        let cap = limit.unwrap_or(1000);
        let mut results: Vec<TraceEvent> = Vec::new();

        for line in content.lines() {
            if line.is_empty() {
                continue;
            }
            let event: TraceEvent = match serde_json::from_str(line) {
                Ok(e) => e,
                Err(_) => continue, // skip malformed lines
            };

            // Filter by task_id
            if let Some(tid) = task_id {
                if event.task_id.as_deref() != Some(tid) {
                    continue;
                }
            }

            // Filter by time
            if let Some(after) = since {
                if event.ts < after {
                    continue;
                }
            }

            results.push(event);
            if results.len() >= cap {
                break;
            }
        }

        Ok(results)
    }

    // ─── Private ─────────────────────────────────────────────────────────────

    /// Rename the active file to `traces-{timestamp}.jsonl` and open a fresh one.
    async fn rotate(&self, inner: &mut Inner) -> Result<()> {
        let ts = Utc::now().format("%Y%m%d-%H%M%S");
        let archive_name = format!("traces-{}.jsonl", ts);
        let active_path = self.telemetry_dir.join("traces.jsonl");
        let archive_path = self.telemetry_dir.join(&archive_name);

        // Flush and drop the current handle before renaming.
        inner.file.flush().await.context("flush before rotate")?;
        tokio::fs::rename(&active_path, &archive_path)
            .await
            .with_context(|| format!("rename trace file to {}", archive_name))?;

        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&active_path)
            .await
            .context("open new traces.jsonl after rotation")?;

        inner.file = new_file;
        inner.bytes_written = 0;
        Ok(())
    }
}
