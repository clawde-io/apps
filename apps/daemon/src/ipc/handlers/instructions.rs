// ipc/handlers/instructions.rs — Instruction graph RPC handlers (Sprint ZZ IG.T02/T03, IL.T06)
//
// RPCs:
//   instructions.compile(target, project_path)
//   instructions.explain(path)
//   instructions.budgetReport(project_path)
//   instructions.import(project_path)
//   instructions.lint(project_path)
//   instructions.propose(from_review_id?)
//   instructions.accept(proposal_id)
//   instructions.dismiss(proposal_id)

use crate::instructions::{
    compiler::{CompileTarget, InstructionCompiler},
    importer::InstructionImporter,
    linter::{lint_nodes, LintNode},
    proposals::ProposalEngine,
};
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

pub async fn compile(ctx: &AppContext, params: Value) -> Result<Value> {
    let target_str = params["target"].as_str().unwrap_or("claude");
    let project_path = params["project_path"].as_str().unwrap_or(".");
    let dry_run = params["dry_run"].as_bool().unwrap_or(false);

    let compiler = InstructionCompiler::new(&ctx.storage);
    let target = CompileTarget::parse(target_str);
    let output = compiler.compile(target, project_path).await?;

    let mut resp = json!({
        "target": target_str,
        "bytes_used": output.bytes_used,
        "budget_bytes": output.budget_bytes,
        "node_count": output.node_count,
        "instruction_hash": output.instruction_hash,
        "over_budget": output.over_budget,
        "near_budget": output.near_budget,
    });

    if dry_run {
        resp["content"] = json!(output.content);
        resp["dry_run"] = json!(true);
    }

    if !dry_run && !output.over_budget {
        // Write to file
        let output_path = format!(
            "{}/{}",
            project_path.trim_end_matches('/'),
            output.target.output_filename()
        );
        tokio::fs::write(&output_path, &output.content).await?;
        resp["written_to"] = json!(output_path);
    }

    Ok(resp)
}

pub async fn explain(ctx: &AppContext, params: Value) -> Result<Value> {
    let path = params["path"].as_str().unwrap_or(".");

    let compiler = InstructionCompiler::new(&ctx.storage);
    let result = compiler.explain(path).await?;

    Ok(json!({
        "path": result.path,
        "nodes": result.nodes.iter().map(|n| json!({
            "id": n.id,
            "scope": n.scope,
            "owner": n.owner,
            "priority": n.priority,
            "preview": n.preview,
        })).collect::<Vec<_>>(),
        "merged_preview": result.merged_content.chars().take(500).collect::<String>(),
        "bytes_used": result.bytes_used,
        "budget_bytes": result.budget_bytes,
        "conflicts": result.conflicts,
    }))
}

pub async fn budget_report(ctx: &AppContext, params: Value) -> Result<Value> {
    let project_path = params["project_path"].as_str().unwrap_or(".");

    let compiler = InstructionCompiler::new(&ctx.storage);
    let claude_out = compiler
        .compile(CompileTarget::Claude, project_path)
        .await?;
    let codex_out = compiler.compile(CompileTarget::Codex, project_path).await?;

    Ok(json!({
        "claude": {
            "bytes_used": claude_out.bytes_used,
            "budget_bytes": claude_out.budget_bytes,
            "pct": claude_out.bytes_used * 100 / claude_out.budget_bytes.max(1),
            "over_budget": claude_out.over_budget,
        },
        "codex": {
            "bytes_used": codex_out.bytes_used,
            "budget_bytes": codex_out.budget_bytes,
            "pct": codex_out.bytes_used * 100 / codex_out.budget_bytes.max(1),
            "over_budget": codex_out.over_budget,
        },
    }))
}

pub async fn import_project(ctx: &AppContext, params: Value) -> Result<Value> {
    let project_path = params["project_path"].as_str().unwrap_or(".");

    let importer = InstructionImporter::new(&ctx.storage);
    let result = importer.import_project(project_path).await?;

    Ok(json!({
        "files_scanned": result.files_scanned,
        "nodes_created": result.nodes_created,
        "nodes_skipped": result.nodes_skipped,
    }))
}

pub async fn lint(ctx: &AppContext, params: Value) -> Result<Value> {
    let project_path = params["project_path"].as_str().unwrap_or(".");
    let budget_bytes: usize = params["budget_bytes"].as_u64().unwrap_or(8192) as usize;

    // Load all nodes
    let rows =
        sqlx::query_as::<_, (String, String)>("SELECT id, content_md FROM instruction_nodes")
            .fetch_all(ctx.storage.pool())
            .await?;

    let nodes: Vec<LintNode> = rows
        .into_iter()
        .map(|(id, content)| LintNode { id, content })
        .collect();

    let report = lint_nodes(&nodes, budget_bytes);

    let _ = project_path; // future: filter by project

    Ok(json!({
        "passed": report.passed,
        "errors": report.errors.len(),
        "warnings": report.warnings.len(),
        "issues": report.errors.iter().chain(report.warnings.iter()).map(|i| json!({
            "severity": format!("{:?}", i.severity).to_lowercase(),
            "rule": i.rule,
            "message": i.message,
            "node_ids": i.node_ids,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn propose(ctx: &AppContext, params: Value) -> Result<Value> {
    let project_path = params["project_path"].as_str().unwrap_or(".");

    let engine = ProposalEngine::new(&ctx.storage);
    let ids = engine.scan_and_propose(project_path).await?;
    let count = ids.len();

    Ok(json!({ "proposal_ids": ids, "count": count }))
}

pub async fn accept(ctx: &AppContext, params: Value) -> Result<Value> {
    let proposal_id = params["proposal_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing proposal_id"))?;

    let engine = ProposalEngine::new(&ctx.storage);
    let node_id = engine.accept(proposal_id).await?;

    Ok(json!({ "node_id": node_id, "accepted": true }))
}

pub async fn dismiss(ctx: &AppContext, params: Value) -> Result<Value> {
    let proposal_id = params["proposal_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing proposal_id"))?;

    let engine = ProposalEngine::new(&ctx.storage);
    engine.dismiss(proposal_id).await?;

    Ok(json!({ "dismissed": true }))
}

/// PT.T03 — instructions.snapshot: compile + write golden snapshot file.
pub async fn snapshot(ctx: &AppContext, params: Value) -> Result<Value> {
    let path = params["path"].as_str().unwrap_or(".");

    let compiler = InstructionCompiler::new(&ctx.storage);
    let output = compiler
        .compile(crate::instructions::compiler::CompileTarget::Claude, path)
        .await?;

    let snap_path = std::path::Path::new(path).join(".instruction-snapshot.md");
    crate::instructions::snapshot::write_snapshot(&output.content, &snap_path).await?;

    Ok(json!({
        "content": output.content,
        "snapshot_path": snap_path.to_string_lossy(),
        "bytes": output.bytes_used,
        "instruction_hash": output.instruction_hash,
    }))
}

/// PT.T03 — instructions.snapshotCheck: diff compiled output against golden file.
pub async fn snapshot_check(ctx: &AppContext, params: Value) -> Result<Value> {
    let path = params["path"].as_str().unwrap_or(".");

    let compiler = InstructionCompiler::new(&ctx.storage);
    let output = compiler
        .compile(crate::instructions::compiler::CompileTarget::Claude, path)
        .await?;

    let snap_path = std::path::Path::new(path).join(".instruction-snapshot.md");
    let (matches, delta) =
        crate::instructions::snapshot::check_snapshot(&output.content, &snap_path).await?;

    Ok(json!({
        "matches": matches,
        "delta": delta,
        "bytes": output.bytes_used,
    }))
}

/// IG.T07 — instructions.doctor: validate compiled instruction files locally.
pub async fn doctor(ctx: &AppContext, params: Value) -> Result<Value> {
    let project_path = params["project_path"].as_str().unwrap_or(".");

    let compiler = InstructionCompiler::new(&ctx.storage);
    let output = compiler
        .compile(
            crate::instructions::compiler::CompileTarget::Claude,
            project_path,
        )
        .await?;

    let mut findings: Vec<serde_json::Value> = Vec::new();

    // Check budget
    if output.over_budget {
        findings.push(json!({
            "kind": "error",
            "message": format!(
                "Compiled output ({} bytes) exceeds budget ({} bytes)",
                output.bytes_used, output.budget_bytes
            ),
            "file": "",
        }));
    } else if output.near_budget {
        findings.push(json!({
            "kind": "warning",
            "message": format!(
                "Instruction budget at {}% — consider trimming nodes",
                output.bytes_used * 100 / output.budget_bytes.max(1)
            ),
            "file": "",
        }));
    }

    // Check for missing CLAUDE.md
    let claude_md = format!("{}/CLAUDE.md", project_path.trim_end_matches('/'));
    if !std::path::Path::new(&claude_md).exists() && output.node_count > 0 {
        findings.push(json!({
            "kind": "warning",
            "message": "CLAUDE.md not found — run `clawd instructions compile` to generate it",
            "file": claude_md,
        }));
    }

    Ok(json!({
        "findings": findings,
        "node_count": output.node_count,
        "bytes_used": output.bytes_used,
        "budget_bytes": output.budget_bytes,
    }))
}
