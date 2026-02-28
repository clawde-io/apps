/// standards.list — return active coding standards for a session's repo.
///
/// V02.T31: Detect language from repo_path, return bundle for UI display.
use crate::standards::{bundle_for, detect_language};
use crate::AppContext;
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

/// standards.list — detect language + return active standards for a project.
///
/// Params: { "project_path": "/path/to/repo" }
/// Returns: { "language": "rust", "standards": "...", "active": true }
pub async fn list(params: Value, _ctx: &AppContext) -> Result<Value> {
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();

    let path = Path::new(&project_path);
    let lang = detect_language(path);
    let standards = bundle_for(&lang);

    Ok(serde_json::json!({
        "language": lang.as_str(),
        "standards": standards,
        "active": standards.is_some(),
    }))
}
