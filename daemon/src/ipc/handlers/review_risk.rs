// ipc/handlers/review_risk.rs — Diff risk score RPC (Sprint ZZ DR.T01)
//
// RPC: review.diffRisk(session_id, worktree_path) → DiffRiskResult

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};


/// File criticality weights for diff risk scoring.
/// Higher = riskier to change.
const CRITICALITY_WEIGHTS: &[(&str, f64)] = &[
    // Security-sensitive patterns
    ("security", 2.0),
    ("auth", 1.9),
    ("crypto", 1.9),
    ("vault", 1.9),
    ("secret", 1.9),
    // Database migrations
    ("migrations/", 1.8),
    ("migration", 1.8),
    // Core daemon
    ("daemon/src/lib.rs", 1.7),
    ("daemon/src/main.rs", 1.6),
    ("daemon/src/ipc/", 1.5),
    ("daemon/src/storage/", 1.5),
    // General source
    ("daemon/src/", 1.3),
    ("packages/", 1.2),
    // Tests (lower risk — they test the system)
    ("tests/", 0.8),
    ("test/", 0.8),
    ("_test.rs", 0.8),
    ("_test.dart", 0.8),
    ("spec.ts", 0.8),
    // Documentation (lowest risk)
    (".md", 0.3),
    (".wiki/", 0.3),
    ("docs/", 0.3),
];

const DEFAULT_WEIGHT: f64 = 1.0;
const CHURN_COEFFICIENT_PER_LINE: f64 = 0.1;
const LINES_THRESHOLD_FOR_CHURN: u32 = 100;

#[derive(Debug)]
struct FileRisk {
    path: String,
    lines_changed: u32,
    criticality_weight: f64,
    risk_score: f64,
    category: &'static str,
}

fn criticality_for_path(path: &str) -> (f64, &'static str) {
    for (pattern, weight) in CRITICALITY_WEIGHTS {
        if path.contains(pattern) {
            return (
                *weight,
                match weight {
                    w if *w >= 1.8 => "critical",
                    w if *w >= 1.4 => "high",
                    w if *w >= 1.0 => "normal",
                    _ => "low",
                },
            );
        }
    }
    (DEFAULT_WEIGHT, "normal")
}

fn compute_churn_coefficient(lines_changed: u32) -> f64 {
    if lines_changed <= LINES_THRESHOLD_FOR_CHURN {
        1.0
    } else {
        1.0 + (lines_changed - LINES_THRESHOLD_FOR_CHURN) as f64 * CHURN_COEFFICIENT_PER_LINE
    }
}

fn compute_file_risk(path: &str, lines_changed: u32) -> FileRisk {
    let (weight, category) = criticality_for_path(path);
    let churn = compute_churn_coefficient(lines_changed);
    let risk_score = lines_changed as f64 * weight * churn;

    FileRisk {
        path: path.to_string(),
        lines_changed,
        criticality_weight: weight,
        risk_score,
        category,
    }
}

/// DR.T01 — `review.diffRisk(session_id, worktree_path)` RPC
pub async fn diff_risk(ctx: &AppContext, params: Value) -> Result<Value> {
    let worktree_path = params["worktree_path"].as_str();
    let session_id = params["session_id"].as_str().unwrap_or("unknown");

    // Load risk thresholds: params override config, config overrides built-in defaults (DR.T02)
    let warn_threshold = params["warn_threshold"]
        .as_f64()
        .unwrap_or(ctx.config.diff_risk.warn_threshold);
    let block_threshold = params["block_threshold"]
        .as_f64()
        .unwrap_or(ctx.config.diff_risk.block_threshold);

    // Collect changed files from worktree
    let file_changes = if let Some(wt_path) = worktree_path {
        collect_file_changes(wt_path).await
    } else {
        Vec::new()
    };

    // Compute risk per file
    let file_risks: Vec<FileRisk> = file_changes
        .iter()
        .map(|(path, lines)| compute_file_risk(path, *lines))
        .collect();

    let total_score: f64 = file_risks.iter().map(|f| f.risk_score).sum();

    // Format response
    let files_json: Vec<Value> = {
        let mut sorted = file_risks.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| b.risk_score.partial_cmp(&a.risk_score).unwrap_or(std::cmp::Ordering::Equal));
        sorted.iter().map(|f| json!({
            "path": f.path,
            "lines_changed": f.lines_changed,
            "criticality_weight": f.criticality_weight,
            "risk_score": f.risk_score,
            "category": f.category,
        })).collect()
    };

    let status = if total_score >= block_threshold {
        "blocked"
    } else if total_score >= warn_threshold {
        "warning"
    } else {
        "ok"
    };

    // Record to audit if risk is high
    if total_score >= warn_threshold {
        let audit_id = uuid::Uuid::new_v4().to_string().replace('-', "");
        let now = chrono::Utc::now().timestamp();
        let meta = serde_json::to_string(&serde_json::json!({
            "total_score": total_score,
            "status": status,
        }))
        .unwrap_or_default();
        let _ = sqlx::query(
            "INSERT INTO audit_log \
             (id, actor_id, action, resource_type, resource_id, metadata_json, created_at) \
             VALUES (?, 'daemon', 'review.diffRisk', 'session', ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(session_id)
        .bind(&meta)
        .bind(now)
        .execute(ctx.storage.pool())
        .await;
    }

    Ok(json!({
        "total_score": total_score,
        "warn_threshold": warn_threshold,
        "block_threshold": block_threshold,
        "status": status,
        "files": files_json,
        "session_id": session_id,
    }))
}

/// Collect changed files + line counts from a git worktree.
async fn collect_file_changes(worktree_path: &str) -> Vec<(String, u32)> {
    let output = tokio::process::Command::new("git")
        .args(["-C", worktree_path, "diff", "--numstat", "HEAD"])
        .output()
        .await;

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines() {
        // Format: "insertions\tdeletions\tfilename"
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let insertions: u32 = parts[0].parse().unwrap_or(0);
            let deletions: u32 = parts[1].parse().unwrap_or(0);
            let filename = parts[2].to_string();
            results.push((filename, insertions + deletions));
        }
    }

    results
}
