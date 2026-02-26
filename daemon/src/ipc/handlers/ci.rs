//! Sprint EE CI.3 — `ci.*` RPC handlers.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// In-memory store of active CI runs.
static CI_RUNS: std::sync::OnceLock<Arc<Mutex<HashMap<String, crate::ci::runner::CiRunStatus>>>> =
    std::sync::OnceLock::new();

fn ci_runs() -> &'static Arc<Mutex<HashMap<String, crate::ci::runner::CiRunStatus>>> {
    CI_RUNS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

/// `ci.run` — Start a CI run for the given repo.
///
/// Params: `{ repoPath: String, step?: String }`
pub async fn run(params: Value, ctx: AppContext) -> Result<Value> {
    let repo_path = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();

    let filter_step = params
        .get("step")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let config = crate::ci::config::load(std::path::Path::new(&repo_path))?;
    let run_id = Uuid::new_v4().to_string();

    // Register run as active
    {
        let mut runs = ci_runs().lock().await;
        runs.insert(
            run_id.clone(),
            crate::ci::runner::CiRunStatus::Running,
        );
    }

    let run_id_clone = run_id.clone();
    let broadcaster = ctx.broadcaster.clone();
    let runs_store = ci_runs().clone();

    // Filter steps if requested
    let mut config = config;
    if let Some(step_name) = filter_step {
        config.steps = config
            .steps
            .into_iter()
            .filter(|s| s.name == step_name)
            .collect();
        if config.steps.is_empty() {
            anyhow::bail!("Step '{step_name}' not found in CI config");
        }
    }

    tokio::spawn(async move {
        let mut ci_run = crate::ci::runner::CiRun::new(config, repo_path);

        let final_status = ci_run
            .execute(|method, params| {
                broadcaster.broadcast(method, params);
            })
            .await
            .unwrap_or(crate::ci::runner::CiRunStatus::Failure);

        // Update stored status
        let mut runs = runs_store.lock().await;
        runs.insert(run_id_clone, final_status);
    });

    Ok(json!({
        "runId": run_id,
        "status": "running",
    }))
}

/// `ci.status` — Get the status of a CI run.
pub async fn status(params: Value, _ctx: AppContext) -> Result<Value> {
    let run_id = params
        .get("runId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("runId required"))?;

    let runs = ci_runs().lock().await;
    if let Some(status) = runs.get(run_id) {
        Ok(json!({
            "runId": run_id,
            "status": status.as_str(),
        }))
    } else {
        anyhow::bail!("Run '{run_id}' not found")
    }
}

/// `ci.cancel` — Cancel a running CI run.
pub async fn cancel(params: Value, ctx: AppContext) -> Result<Value> {
    let run_id = params
        .get("runId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("runId required"))?;

    {
        let mut runs = ci_runs().lock().await;
        if runs.contains_key(run_id) {
            runs.insert(
                run_id.to_string(),
                crate::ci::runner::CiRunStatus::Canceled,
            );
        } else {
            anyhow::bail!("Run '{run_id}' not found");
        }
    }

    ctx.broadcaster.broadcast(
        "ci.complete",
        json!({
            "runId": run_id,
            "status": "canceled",
        }),
    );

    Ok(json!({ "runId": run_id, "status": "canceled" }))
}
