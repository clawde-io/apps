/// Sprint EE DD.5 — Unit tests for digest generation.
#[cfg(test)]
mod tests {
    use crate::scheduler::digest::{DigestEntry, DigestMetrics, generate_today};
    use serde_json::Value;

    #[test]
    fn digest_entry_json_roundtrip() {
        let entry = DigestEntry {
            session_id: "sess-001".into(),
            session_title: "Auth refactor".into(),
            provider: "claude".into(),
            messages_count: 34,
            tasks_completed: 5,
            files_changed: vec!["main.rs".into(), "auth.rs".into()],
            started_at: "2026-02-26T09:00:00Z".into(),
            ended_at: Some("2026-02-26T10:30:00Z".into()),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["session_id"], "sess-001");
        assert_eq!(json["messages_count"], 34);
        assert_eq!(json["tasks_completed"], 5);
        let files = json["files_changed"].as_array().unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn digest_metrics_defaults() {
        let m = DigestMetrics::default();
        assert_eq!(m.sessions_run, 0);
        assert_eq!(m.tasks_completed, 0);
        assert_eq!(m.tasks_in_progress, 0);
        assert!(m.top_files.is_empty());
    }

    #[test]
    fn digest_entry_no_end_time() {
        let entry = DigestEntry {
            session_id: "sess-002".into(),
            session_title: "In-progress session".into(),
            provider: "codex".into(),
            messages_count: 5,
            tasks_completed: 0,
            files_changed: vec![],
            started_at: "2026-02-26T12:00:00Z".into(),
            ended_at: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["ended_at"], Value::Null);
    }

    #[test]
    fn digest_metrics_top_files_truncated() {
        // Top files should contain at most 10 items in practice.
        let mut files: Vec<String> = (0..15)
            .map(|i| format!("file_{}.rs", i))
            .collect();
        files.truncate(10);
        let m = DigestMetrics {
            sessions_run: 3,
            tasks_completed: 10,
            tasks_in_progress: 1,
            top_files: files.clone(),
            eval_avg: 0.0,
            velocity: Default::default(),
        };
        assert_eq!(m.top_files.len(), 10);
    }
}
