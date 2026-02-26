// memory/extractor.rs — Post-session memory extraction.
//
// Sprint OO ME.4: After a session completes, extract key learnings into memory.
//
// Extraction uses a lightweight model call to summarize the session:
//   System: "Extract up to 5 key facts learned in this session as JSON."
//   User: <session transcript summary>
//
// The response is parsed and inserted into the memory store.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::memory::store::{AddMemoryRequest, MemoryStore};

#[derive(Debug, Deserialize, Serialize)]
struct ExtractedFact {
    key: String,
    value: String,
    weight: Option<i64>,
}

/// Summary of a completed session to feed into the extractor.
pub struct SessionSummary {
    pub session_id: String,
    pub repo_path: String,
    pub message_count: usize,
    /// Key messages from the session (last N turns or summary)
    pub content_preview: String,
}

/// Extract memory entries from a completed session.
///
/// This makes a lightweight model call to identify reusable facts.
/// The extraction model is typically a smaller/faster model (e.g., claude-haiku-4-5).
///
/// In phase OO this is a stub — real model call wired when integration tests pass.
pub async fn extract_memories(
    store: &MemoryStore,
    session: &SessionSummary,
    extraction_model: &str,
) -> Result<Vec<String>> {
    // Production: call extraction model with the session preview
    // For now: pattern-match common memory-worthy content from session

    let mut extracted_keys = Vec::new();

    // Simple heuristic extraction (no model call) for MVP:
    // Look for explicit user statements about preferences
    let patterns = [
        ("language", &["prefer Rust", "use Rust", "Rust only"][..]),
        ("style.verbosity", &["keep it brief", "be concise", "short responses"]),
        ("testing.framework", &["use jest", "use pytest", "use cargo test"]),
        ("style.comments", &["add comments", "no comments", "minimal comments"]),
    ];

    let content = &session.content_preview;
    let project_scope = MemoryStore::project_scope(&session.repo_path);

    for (key, phrases) in &patterns {
        for phrase in *phrases {
            if content.to_lowercase().contains(phrase) {
                let result = store
                    .upsert(AddMemoryRequest {
                        scope: project_scope.clone(),
                        key: key.to_string(),
                        value: phrase.to_string(),
                        weight: Some(6),
                        source: Some(format!("auto:session:{}", &session.session_id[..8])),
                    })
                    .await;

                if let Ok(entry) = result {
                    extracted_keys.push(entry.key);
                    break; // Only one match per key
                }
            }
        }
    }

    tracing::debug!(
        session_id = %session.session_id,
        extracted = extracted_keys.len(),
        model = extraction_model,
        "Memory extraction complete"
    );

    Ok(extracted_keys)
}
