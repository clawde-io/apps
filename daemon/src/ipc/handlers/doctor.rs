// SPDX-License-Identifier: MIT
//! RPC handlers for `doctor.*` methods (D64.T02).
//!
//! Exposes:
//!   `doctor.scan`            — scan project for AFS/docs/release health issues
//!   `doctor.fix`             — auto-fix fixable findings
//!   `doctor.approveRelease`  — mark a release plan as approved
//!   `doctor.hookInstall`     — install git pre-tag hook

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `doctor.scan` — scan a project for health issues.
///
/// Params: `{ project_path: string, scope?: "afs"|"docs"|"release"|"all" }`
/// Returns: `{ score: u8, findings: [ DoctorFinding ] }`
pub async fn scan(params: Value, _ctx: &AppContext) -> Result<Value> {
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: project_path"))?;

    let path = std::path::Path::new(project_path);
    if !path.is_absolute() {
        anyhow::bail!("project_path must be absolute");
    }
    if !path.exists() {
        anyhow::bail!("project_path does not exist: {}", project_path);
    }

    let scope_str = params
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("all");
    let scope = crate::doctor::ScanScope::from_str(scope_str);

    let result = crate::doctor::scan(path, scope);
    let findings_json: Vec<Value> = result
        .findings
        .iter()
        .map(|f| {
            json!({
                "code": f.code,
                "severity": f.severity,
                "message": f.message,
                "path": f.path,
                "fixable": f.fixable,
            })
        })
        .collect();

    Ok(json!({
        "score": result.score,
        "findings": findings_json,
    }))
}

/// `doctor.fix` — apply auto-fixable repairs.
///
/// Params: `{ project_path: string, finding_codes?: [string] }`
/// Returns: `{ fixed: [string], skipped: [string] }`
pub async fn fix(params: Value, _ctx: &AppContext) -> Result<Value> {
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: project_path"))?;

    let path = std::path::Path::new(project_path);
    if !path.is_absolute() {
        anyhow::bail!("project_path must be absolute");
    }

    let codes: Vec<String> = params
        .get("finding_codes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let result = crate::doctor::fix(path, &codes);
    Ok(json!({
        "fixed": result.fixed,
        "skipped": result.skipped,
    }))
}

/// `doctor.approveRelease` — mark a release plan as approved.
///
/// Params: `{ version: string, project_path: string }`
/// Returns: `{ ok: bool }`
pub async fn approve_release(params: Value, _ctx: &AppContext) -> Result<Value> {
    let version = params
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: version"))?;
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: project_path"))?;

    let path = std::path::Path::new(project_path);
    let ok = crate::doctor::approve_release(path, version);
    Ok(json!({ "ok": ok }))
}

/// `doctor.hookInstall` — install git pre-tag release lock hook.
///
/// Params: `{ project_path: string }`
/// Returns: `{ ok: bool }`
pub async fn hook_install(params: Value, _ctx: &AppContext) -> Result<Value> {
    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field: project_path"))?;

    let path = std::path::Path::new(project_path);
    let ok = crate::doctor::release_checks::install_pre_tag_hook(path).is_ok();
    Ok(json!({ "ok": ok }))
}
