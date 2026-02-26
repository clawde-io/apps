// cli/instructions.rs — Instructions CLI commands (Sprint ZZ IG.T05, IG.T07, IL.T03)
//
// clawd instructions compile [--target claude|codex|all] [--project <path>] [--dry-run]
// clawd instructions explain --path <dir>
// clawd instructions lint [--ci]
// clawd instructions import [--project <path>]
// clawd instructions snapshot [--path <dir>] [--check]
// clawd doctor --instructions

use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Options for `clawd instructions compile`.
pub struct CompileOpts {
    pub target: String,
    pub project: PathBuf,
    pub dry_run: bool,
}

/// IG.T05 — `clawd instructions compile`
pub async fn compile(opts: CompileOpts, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client
        .call_once(
            "instructions.compile",
            json!({
                "target": opts.target,
                "project_path": opts.project.to_string_lossy(),
                "dry_run": opts.dry_run,
            }),
        )
        .await?;

    if opts.dry_run {
        let content = result["content"].as_str().unwrap_or("");
        println!("=== Dry run — compiled output for {} ===", opts.target);
        println!("{content}");
    } else {
        let written = result["written_to"].as_str().unwrap_or("(none)");
        let bytes = result["bytes_used"].as_u64().unwrap_or(0);
        let budget = result["budget_bytes"].as_u64().unwrap_or(8192);
        let pct = bytes * 100 / budget.max(1);
        let over = result["over_budget"].as_bool().unwrap_or(false);
        let near = result["near_budget"].as_bool().unwrap_or(false);

        if over {
            eprintln!(
                "ERROR: Compiled output ({bytes}B) exceeds budget ({budget}B). \
                 Split rules into smaller nodes.",
            );
            std::process::exit(1);
        }

        if near {
            eprintln!("WARNING: Instruction budget at {pct}% — consider trimming nodes.");
        }

        println!("Compiled → {written}");
        println!("  {} bytes / {} budget ({pct}%)", bytes, budget);
    }

    Ok(())
}

/// `clawd instructions explain --path <dir>`
pub async fn explain(path: PathBuf, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client
        .call_once(
            "instructions.explain",
            json!({ "path": path.to_string_lossy() }),
        )
        .await?;

    let nodes = result["nodes"].as_array().cloned().unwrap_or_default();
    println!("Effective instructions for: {}", path.display());
    println!("  {} node(s)", nodes.len());
    for node in &nodes {
        let id = node["id"].as_str().unwrap_or("?");
        let scope = node["scope"].as_str().unwrap_or("?");
        let owner = node["owner"].as_str().unwrap_or("-");
        let priority = node["priority"].as_i64().unwrap_or(100);
        println!("  [{scope:8} pri={priority:3}] {id} (owner: {owner})");
    }

    let conflicts = result["conflicts"].as_array().cloned().unwrap_or_default();
    if !conflicts.is_empty() {
        eprintln!("\nConflicts detected:");
        for c in &conflicts {
            eprintln!("  ⚠ {}", c.as_str().unwrap_or("?"));
        }
    }

    let bytes = result["bytes_used"].as_u64().unwrap_or(0);
    let budget = result["budget_bytes"].as_u64().unwrap_or(8192);
    println!("\nBudget: {} / {} bytes", bytes, budget);
    Ok(())
}

/// IL.T03 — `clawd instructions lint [--ci]`
pub async fn lint(project: PathBuf, ci: bool, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client
        .call_once(
            "instructions.lint",
            json!({ "project_path": project.to_string_lossy() }),
        )
        .await?;

    let passed = result["passed"].as_bool().unwrap_or(false);
    let errors = result["errors"].as_u64().unwrap_or(0);
    let warnings = result["warnings"].as_u64().unwrap_or(0);
    let issues = result["issues"].as_array().cloned().unwrap_or_default();

    for issue in &issues {
        let severity = issue["severity"].as_str().unwrap_or("warning");
        let rule = issue["rule"].as_str().unwrap_or("?");
        let message = issue["message"].as_str().unwrap_or("?");
        let prefix = if severity == "error" { "ERROR" } else { "WARN " };
        eprintln!("{prefix} [{rule}] {message}");
    }

    if ci {
        // CI mode: summary line on stdout for log parsing
        println!(
            "instruction-lint: {} errors, {} warnings — {}",
            errors,
            warnings,
            if passed { "PASS" } else { "FAIL" }
        );
    } else {
        println!(
            "Lint: {} errors, {} warnings — {}",
            errors,
            warnings,
            if passed { "✓ PASS" } else { "✗ FAIL" }
        );
    }

    if !passed {
        std::process::exit(1);
    }
    Ok(())
}

/// `clawd instructions import [--project <path>]`
pub async fn import(project: PathBuf, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client
        .call_once(
            "instructions.import",
            json!({ "project_path": project.to_string_lossy() }),
        )
        .await?;

    let scanned = result["files_scanned"].as_u64().unwrap_or(0);
    let created = result["nodes_created"].as_u64().unwrap_or(0);
    let skipped = result["nodes_skipped"].as_u64().unwrap_or(0);

    println!(
        "Import complete: {} files scanned, {} nodes created, {} skipped (already present)",
        scanned, created, skipped
    );
    Ok(())
}

/// PT.T03 — `clawd instructions snapshot [--path <dir>] [--check]`
pub async fn snapshot(
    path: PathBuf,
    output: Option<PathBuf>,
    check: bool,
    data_dir: &Path,
    port: u16,
) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    if check {
        let result = client
            .call_once(
                "instructions.snapshotCheck",
                json!({ "path": path.to_string_lossy() }),
            )
            .await?;

        let matches = result["matches"].as_bool().unwrap_or(false);
        let delta = result["delta"].as_str().unwrap_or("");

        if matches {
            println!("Snapshot check: OK — instructions match golden file.");
        } else {
            eprintln!("Snapshot check: FAIL — instructions have drifted from golden.");
            if !delta.is_empty() {
                eprintln!("\nDiff:\n{delta}");
            }
            std::process::exit(1);
        }
    } else {
        let result = client
            .call_once(
                "instructions.snapshot",
                json!({ "path": path.to_string_lossy() }),
            )
            .await?;

        let content = result["content"].as_str().unwrap_or("");
        let out_path = output.unwrap_or_else(|| path.join(".instruction-snapshot.md"));
        tokio::fs::write(&out_path, content).await?;
        println!("Snapshot written to: {}", out_path.display());
    }
    Ok(())
}

/// IG.T07 — `clawd doctor --instructions` subcommand
///
/// Validates compiled instruction files locally (no daemon required).
pub async fn doctor(project: PathBuf, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client
        .call_once(
            "instructions.doctor",
            json!({ "project_path": project.to_string_lossy() }),
        )
        .await?;

    let findings = result["findings"].as_array().cloned().unwrap_or_default();
    let ok = findings.is_empty();

    if ok {
        println!("Instructions doctor: OK — no issues found.");
    } else {
        eprintln!("Instructions doctor: {} issue(s) found:", findings.len());
        for f in &findings {
            let kind = f["kind"].as_str().unwrap_or("issue");
            let msg = f["message"].as_str().unwrap_or("?");
            let file = f["file"].as_str().unwrap_or("");
            if file.is_empty() {
                eprintln!("  [{kind}] {msg}");
            } else {
                eprintln!("  [{kind}] {file}: {msg}");
            }
        }
        std::process::exit(1);
    }
    Ok(())
}
