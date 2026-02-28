// cli/bench.rs — Benchmark CLI (Sprint ZZ EH.T03, EH.T04)
//
// clawd bench run [--task <id>] [--provider <name>]
// clawd bench compare --base <git-ref>

use anyhow::Result;
use serde_json::json;
use std::path::Path;

/// EH.T03 — `clawd bench run [--task <id>] [--provider <name>]`
pub async fn run(
    task_id: Option<String>,
    provider: Option<String>,
    data_dir: &Path,
    port: u16,
) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let mut params = json!({});
    if let Some(tid) = task_id {
        params["task_id"] = json!(tid);
    }
    if let Some(p) = provider {
        params["provider"] = json!(p);
    }

    println!("Starting benchmark run...");
    let result = client.call_once("bench.run", params).await?;

    let runs = result["runs"].as_array().cloned().unwrap_or_default();
    let total = runs.len();
    let passed = runs
        .iter()
        .filter(|r| r["success"].as_bool().unwrap_or(false))
        .count();

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║ Benchmark Results                                        ║");
    println!("╠══════════════╦══════════╦═══════════╦═══════════════════╣");
    println!("║ Task ID      ║ Success  ║ Turns     ║ Duration          ║");
    println!("╠══════════════╬══════════╬═══════════╬═══════════════════╣");

    for run in &runs {
        let task = run["task_id"].as_str().unwrap_or("?");
        let success = run["success"].as_bool().unwrap_or(false);
        let turns = run["turns"].as_u64().unwrap_or(0);
        let duration_ms = run["duration_ms"].as_u64().unwrap_or(0);
        let success_str = if success { "✓" } else { "✗" };
        println!(
            "║ {:<12} ║ {:<8} ║ {:<9} ║ {:<17} ║",
            &task[..task.len().min(12)],
            success_str,
            turns,
            format!("{duration_ms}ms")
        );
    }

    println!("╚══════════════╩══════════╩═══════════╩═══════════════════╝");
    println!(
        "\nTotal: {total} | Passed: {passed} | Failed: {}",
        total - passed
    );
    println!(
        "Pass rate: {:.1}%",
        passed as f64 / total.max(1) as f64 * 100.0
    );

    if passed < total {
        std::process::exit(1);
    }

    Ok(())
}

/// EH.T04 — `clawd bench compare --base <git-ref>`
pub async fn compare(base_ref: String, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    println!("Running benchmark comparison against base: {base_ref}");

    let result = client
        .call_once("bench.compare", json!({ "base_ref": base_ref }))
        .await?;

    let current_pass_rate = result["current_pass_rate"].as_f64().unwrap_or(0.0);
    let base_pass_rate = result["base_pass_rate"].as_f64().unwrap_or(0.0);
    let current_turns_mean = result["current_turns_mean"].as_f64().unwrap_or(0.0);
    let base_turns_mean = result["base_turns_mean"].as_f64().unwrap_or(0.0);
    let current_diff_lines_mean = result["current_diff_lines_mean"].as_f64().unwrap_or(0.0);
    let base_diff_lines_mean = result["base_diff_lines_mean"].as_f64().unwrap_or(0.0);

    let pass_delta = current_pass_rate - base_pass_rate;
    let turns_delta = current_turns_mean - base_turns_mean;

    println!("\nComparison: HEAD vs {base_ref}");
    println!("─────────────────────────────────────────────");
    println!("Metric           │ Base     │ Current  │ Delta");
    println!("─────────────────┼──────────┼──────────┼───────────");
    println!(
        "Pass rate        │ {base_pass_rate:>6.1}%   │ {current_pass_rate:>6.1}%   │ {:>+8.1}%",
        pass_delta
    );
    println!(
        "Mean turns       │ {base_turns_mean:>8.1} │ {current_turns_mean:>8.1} │ {:>+10.1}",
        turns_delta
    );
    println!("Mean diff lines  │ {base_diff_lines_mean:>8.1} │ {current_diff_lines_mean:>8.1} │",);
    println!("─────────────────────────────────────────────");

    // EH.T06 — Regression gate thresholds
    let regressions: Vec<String> = {
        let mut r = Vec::new();
        if pass_delta < -5.0 {
            r.push(format!(
                "Pass rate regressed by {:.1}% (threshold: 5%)",
                -pass_delta
            ));
        }
        if turns_delta > 20.0 / 100.0 * base_turns_mean {
            r.push(format!(
                "Mean turns increased by {turns_delta:.1} (>20% of baseline {base_turns_mean:.1})"
            ));
        }
        r
    };

    if !regressions.is_empty() {
        eprintln!("\nREGRESSIONS DETECTED:");
        for r in &regressions {
            eprintln!("  ✗ {r}");
        }
        std::process::exit(1);
    } else {
        println!("\n✓ No regressions detected.");
    }

    Ok(())
}
