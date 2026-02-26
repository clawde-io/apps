// cli/diff_risk.rs — `clawd diff-risk-score` (Sprint ZZ DR.T04)
//
// Shows current diff risk score + breakdown by file.

use anyhow::Result;
use serde_json::json;
use std::path::Path;

/// DR.T04 — `clawd diff-risk-score [--path <worktree>]`
pub async fn diff_risk_score(
    worktree_path: Option<String>,
    data_dir: &Path,
    port: u16,
) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let mut params = json!({});
    if let Some(p) = worktree_path {
        params["worktree_path"] = json!(p);
    }

    let result = client.call_once("review.diffRisk", params).await?;

    let total_score = result["total_score"].as_f64().unwrap_or(0.0);
    let warn_threshold = result["warn_threshold"].as_f64().unwrap_or(50.0);
    let block_threshold = result["block_threshold"].as_f64().unwrap_or(200.0);
    let files = result["files"].as_array().cloned().unwrap_or_default();

    // Status indicator
    let status = if total_score >= block_threshold {
        "BLOCKED ✗"
    } else if total_score >= warn_threshold {
        "WARNING ⚠"
    } else {
        "OK ✓"
    };

    println!("Diff Risk Score: {total_score:.1} — {status}");
    println!(
        "  Thresholds: warn={warn_threshold:.0}, block={block_threshold:.0}"
    );

    if !files.is_empty() {
        println!("\nFile breakdown (highest risk first):");
        println!("  {:<50} {:>8}  {}", "File", "Score", "Category");
        println!("  {}", "─".repeat(70));

        let mut sorted = files.clone();
        sorted.sort_by(|a, b| {
            b["risk_score"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&a["risk_score"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for file in &sorted {
            let path = file["path"].as_str().unwrap_or("?");
            let score = file["risk_score"].as_f64().unwrap_or(0.0);
            let category = file["category"].as_str().unwrap_or("normal");
            let path_short = if path.len() > 50 {
                &path[path.len() - 50..]
            } else {
                path
            };
            println!("  {path_short:<50} {score:>8.1}  {category}");
        }
    }

    // Suggestions for reducing score
    if total_score >= warn_threshold {
        println!("\nTo reduce score, consider splitting this change into smaller tasks.");
        println!("Use `clawd task expand-ownership` only for files within task scope.");
    }

    if total_score >= block_threshold {
        std::process::exit(1);
    }

    Ok(())
}
