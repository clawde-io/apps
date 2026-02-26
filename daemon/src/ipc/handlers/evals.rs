//! Sprint CC EV.6 — `eval.*` RPC handlers.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `eval.list` — list available eval files in `.claw/evals/`.
pub async fn eval_list(params: Value, _ctx: AppContext) -> Result<Value> {
    let repo_path = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let evals_dir = std::path::Path::new(repo_path).join(".claw").join("evals");
    let mut files: Vec<String> = Vec::new();

    if evals_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&evals_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        files.push(name.to_string());
                    }
                }
            }
        }
    }

    // Also include built-in evals reference.
    files.push("builtin_evals.yaml (built-in)".to_string());

    Ok(json!({ "files": files, "evalsDir": evals_dir.to_string_lossy() }))
}

/// `eval.run` — run evals from a YAML file against the current session config.
pub async fn eval_run(params: Value, ctx: AppContext) -> Result<Value> {
    let repo_path = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let eval_file = params
        .get("file")
        .and_then(|v| v.as_str())
        .unwrap_or("builtin_evals.yaml");
    let threshold = params
        .get("threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let results =
        crate::evals::session_eval::run_evals(repo_path, eval_file, &ctx).await?;

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let score = if total > 0 {
        passed as f64 / total as f64
    } else {
        1.0
    };
    let ci_pass = score >= threshold;

    Ok(json!({
        "file": eval_file,
        "total": total,
        "passed": passed,
        "failed": total - passed,
        "score": score,
        "ciPass": ci_pass,
        "results": results.iter().map(|r| json!({
            "name": r.name,
            "passed": r.passed,
            "score": r.score,
            "reason": r.reason,
        })).collect::<Vec<_>>(),
    }))
}
