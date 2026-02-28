// SPDX-License-Identifier: MIT
//! RPC handlers for Builder Mode.
//!
//! Exposed methods:
//!   `builder.createSession` — scaffold a new project from a stack template
//!   `builder.listTemplates` — list available stack templates
//!   `builder.getStatus`     — query the status of a builder session

use super::{model::BuilderSession, model::BuilderStatus, templates};
use crate::AppContext;
use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tracing::{info, warn};
use uuid::Uuid;

/// In-memory store for active builder sessions.
///
/// Builder sessions are ephemeral — they live only while the daemon is running.
/// Persistence is not required because a session's output is the directory on
/// disk; if the daemon restarts, the user simply opens the generated directory.
static SESSIONS: OnceLock<Arc<RwLock<HashMap<String, BuilderSession>>>> = OnceLock::new();

fn sessions() -> Arc<RwLock<HashMap<String, BuilderSession>>> {
    SESSIONS
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

// ─── builder.createSession ───────────────────────────────────────────────────

/// `builder.createSession` — start a new builder session.
///
/// Params:
/// ```json
/// {
///   "stack":       "react-vite",
///   "description": "A todo app with dark mode",
///   "output_dir":  "/Users/alice/projects/todo-app"
/// }
/// ```
///
/// Returns the new `BuilderSession` serialised as JSON.
pub async fn builder_create_session(params: Value, _ctx: &AppContext) -> Result<Value> {
    let stack = params
        .get("stack")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: stack"))?;

    let description = params
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let output_dir = params
        .get("output_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: output_dir"))?
        .to_string();

    let template = templates::find_template(stack)
        .ok_or_else(|| anyhow::anyhow!("unknown stack template: {stack}"))?;

    let id = Uuid::new_v4().to_string();
    let session = BuilderSession {
        id: id.clone(),
        target_stack: stack.to_string(),
        template_name: template.name.clone(),
        status: BuilderStatus::Planning,
        output_dir: output_dir.clone(),
        description: description.clone(),
        files_written: Vec::new(),
        error: None,
        created_at: Utc::now().to_rfc3339(),
    };

    // Store before spawning so callers can poll immediately.
    {
        let store = sessions();
        let mut map = store
            .write()
            .map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        map.insert(id.clone(), session.clone());
    }

    // Spawn scaffold task in background so the RPC returns immediately.
    let output_dir_bg = output_dir.clone();
    let session_id = id.clone();
    let store_bg = sessions();

    tokio::spawn(async move {
        let result = write_template_files(&output_dir_bg, &template.files).await;

        let mut map = match store_bg.write() {
            Ok(m) => m,
            Err(e) => {
                warn!(err = %e, "builder session store poisoned in background task");
                return;
            }
        };

        if let Some(s) = map.get_mut(&session_id) {
            match result {
                Ok(written) => {
                    s.files_written = written;
                    s.status = BuilderStatus::Done;
                    info!(session_id = %session_id, "builder session complete");
                }
                Err(e) => {
                    s.status = BuilderStatus::Failed;
                    s.error = Some(e.to_string());
                    warn!(session_id = %session_id, err = %e, "builder session failed");
                }
            }
        }
    });

    Ok(serde_json::to_value(&session)?)
}

/// Write all template files to `output_dir`, creating parent directories as needed.
/// Returns the list of relative paths written.
async fn write_template_files(
    output_dir: &str,
    files: &[super::model::TemplateFile],
) -> Result<Vec<String>> {
    let base = std::path::Path::new(output_dir);
    tokio::fs::create_dir_all(base).await?;

    let mut written = Vec::new();
    for file in files {
        let dest = base.join(&file.path);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&dest, &file.content).await?;
        written.push(file.path.clone());
    }
    Ok(written)
}

// ─── builder.listTemplates ───────────────────────────────────────────────────

/// `builder.listTemplates` — return all available stack templates.
///
/// Params: none required.
///
/// Returns: array of `{ name, description, file_count }`.
pub async fn builder_list_templates(_params: Value, _ctx: &AppContext) -> Result<Value> {
    let templates = templates::all_templates();
    let list: Vec<Value> = templates
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "file_count": t.files.len(),
            })
        })
        .collect();
    Ok(json!({ "templates": list }))
}

// ─── builder.getStatus ───────────────────────────────────────────────────────

/// `builder.getStatus` — return the current state of a builder session.
///
/// Params: `{ "id": "<session-id>" }`
///
/// Returns the full `BuilderSession` JSON, or an error if not found.
pub async fn builder_get_status(params: Value, _ctx: &AppContext) -> Result<Value> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: id"))?;

    let store = sessions();
    let map = store
        .read()
        .map_err(|_| anyhow::anyhow!("session store poisoned"))?;
    let session = map
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("builder session not found: {id}"))?;

    Ok(serde_json::to_value(session)?)
}
