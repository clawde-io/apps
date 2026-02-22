use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

/// Append-only JSONL event log for a session.
pub struct EventLog {
    path: PathBuf,
}

impl EventLog {
    pub fn new(data_dir: &Path, session_id: &str) -> Self {
        let path = data_dir
            .join("sessions")
            .join(format!("{}.jsonl", session_id));
        Self { path }
    }

    pub async fn append(&self, event: &serde_json::Value) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        let line = serde_json::to_string(event)? + "\n";
        file.write_all(line.as_bytes()).await?;
        Ok(())
    }
}
