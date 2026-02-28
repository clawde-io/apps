/// providers.detect and providers.list RPC handlers — V02.T34-T35.
use crate::providers_knowledge::{bundle_for_provider, detect_providers};
use crate::AppContext;
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

/// providers.detect — detect cloud providers in a project.
///
/// Params: { "project_path": "/path/to/repo" }
/// Returns: { "providers": ["hetzner", "stripe", ...] }
pub async fn detect(params: Value, _ctx: &AppContext) -> Result<Value> {
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();

    let providers = detect_providers(Path::new(&project_path));
    let names: Vec<&str> = providers.iter().map(|p| p.as_str()).collect();

    Ok(serde_json::json!({
        "providers": names,
        "count": names.len(),
    }))
}

/// providers.list — list detected providers with their knowledge bundles.
///
/// Params: { "project_path": "/path/to/repo" }
/// Returns: { "providers": [{ "id": "stripe", "name": "Stripe", "knowledge": "..." }, ...] }
pub async fn list(params: Value, _ctx: &AppContext) -> Result<Value> {
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();

    let providers = detect_providers(Path::new(&project_path));

    let items: Vec<Value> = providers
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.as_str(),
                "name": p.display_name(),
                "knowledge": bundle_for_provider(p),
            })
        })
        .collect();

    let count = items.len();
    Ok(serde_json::json!({
        "providers": items,
        "count": count,
    }))
}
