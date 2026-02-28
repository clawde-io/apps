// ipc/handlers/artifacts.rs — Artifacts RPC handlers (Sprint ZZ EP.T04)
//
// RPCs:
//   artifacts.evidencePack(task_id) → EvidencePack

use crate::tasks::evidence::get_evidence_pack;
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// EP.T04 — `artifacts.evidencePack(task_id)` RPC
pub async fn evidence_pack(ctx: &AppContext, params: Value) -> Result<Value> {
    let task_id = params["task_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing task_id"))?;

    match get_evidence_pack(&ctx.storage, task_id).await? {
        None => {
            anyhow::bail!(
                "No evidence pack found for task '{task_id}'. \
                           The task may not be completed or no evidence was recorded."
            )
        }
        Some(pack) => Ok(json!({
            "task_id": pack.task_id,
            "run_id": pack.run_id,
            "instruction_hash": pack.instruction_hash,
            "policy_hash": pack.policy_hash,
            "worktree_commit": pack.worktree_commit,
            "diff_stats": {
                "files_changed": pack.diff_stats.files_changed,
                "insertions": pack.diff_stats.insertions,
                "deletions": pack.diff_stats.deletions,
                "files": pack.diff_stats.files,
            },
            "test_results": {
                "passed": pack.test_results.passed,
                "failed": pack.test_results.failed,
                "skipped": pack.test_results.skipped,
                "duration_ms": pack.test_results.duration_ms,
                "first_failure": pack.test_results.first_failure,
            },
            "tool_trace_count": pack.tool_trace.len(),
            "tool_trace": pack.tool_trace.iter().map(|t| json!({
                "tool": t.tool,
                "path": t.path,
                "decision": t.decision,
                "duration_ms": t.duration_ms,
            })).collect::<Vec<_>>(),
            "reviewer_verdict": pack.reviewer_verdict,
            "created_at": pack.created_at,
        })),
    }
}
