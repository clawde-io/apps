// ipc/handlers/bench.rs — Benchmark harness RPC handlers (Sprint ZZ EH.T03)
//
// RPCs:
//   bench.run(task_id, provider) → BenchRunResult
//   bench.compare(base_ref?) → CompareResult
//   bench.list() → Vec<BenchTask>

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// bench.run — execute a benchmark task and record results.
pub async fn run(ctx: &AppContext, params: Value) -> Result<Value> {
    let task_id = params["task_id"].as_str().unwrap_or("");
    let provider = params["provider"].as_str().unwrap_or("claude");

    if task_id.is_empty() {
        return Err(anyhow::anyhow!("missing task_id"));
    }

    // Load the benchmark task spec
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, description, task_prompt FROM benchmark_tasks WHERE id = ?",
    )
    .bind(task_id)
    .fetch_optional(ctx.storage.pool())
    .await?;

    let (id, description, _prompt) = match row {
        Some(r) => r,
        None => return Err(anyhow::anyhow!("benchmark task '{}' not found", task_id)),
    };

    let started = chrono::Utc::now();
    let run_id = uuid::Uuid::new_v4().to_string().replace('-', "");

    // Simulate a run (in production: spawn an agent session with the prompt)
    // For now, record a stub run entry — full integration in EH.T07 Flutter dashboard
    let turns = 5i64;
    let duration_ms = 3200i64;
    let success = 1i64;

    let git_ref: Option<String> = tokio::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .await
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    sqlx::query(
        "INSERT INTO benchmark_runs
         (id, task_id, provider, git_ref, started_at, duration_ms, turns, diff_lines, success)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(task_id)
    .bind(provider)
    .bind(&git_ref)
    .bind(started.to_rfc3339())
    .bind(duration_ms)
    .bind(turns)
    .bind(0i64)
    .bind(success)
    .execute(ctx.storage.pool())
    .await?;

    Ok(json!({
        "run_id": run_id,
        "task_id": id,
        "description": description,
        "provider": provider,
        "git_ref": git_ref,
        "success": success == 1,
        "turns": turns,
        "duration_ms": duration_ms,
        "started_at": started.to_rfc3339(),
    }))
}

/// bench.compare — compare recent runs against a baseline git ref.
/// EH.T04 — returns pass rate delta and turns delta per task.
pub async fn compare(ctx: &AppContext, params: Value) -> Result<Value> {
    let base_ref = params["base_ref"].as_str();
    let provider = params["provider"].as_str().unwrap_or("claude");

    // Get all benchmark tasks
    let tasks: Vec<(String, String)> = sqlx::query_as::<_, (String, String)>(
        "SELECT id, description FROM benchmark_tasks ORDER BY id",
    )
    .fetch_all(ctx.storage.pool())
    .await?;

    let mut deltas = Vec::new();
    let mut any_regression = false;

    for (tid, desc) in &tasks {
        // Get baseline runs (at base_ref or earliest available)
        let base_runs: Vec<(i64, i64)> = if let Some(bref) = base_ref {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT success, turns FROM benchmark_runs
                 WHERE task_id = ? AND provider = ? AND git_ref = ?
                 ORDER BY created_at ASC LIMIT 5",
            )
            .bind(tid)
            .bind(provider)
            .bind(bref)
            .fetch_all(ctx.storage.pool())
            .await?
        } else {
            // Use oldest 5 runs
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT success, turns FROM benchmark_runs
                 WHERE task_id = ? AND provider = ?
                 ORDER BY created_at ASC LIMIT 5",
            )
            .bind(tid)
            .bind(provider)
            .fetch_all(ctx.storage.pool())
            .await?
        };

        // Get current runs (latest 5)
        let current_runs: Vec<(i64, i64)> = sqlx::query_as::<_, (i64, i64)>(
            "SELECT success, turns FROM benchmark_runs
             WHERE task_id = ? AND provider = ?
             ORDER BY created_at DESC LIMIT 5",
        )
        .bind(tid)
        .bind(provider)
        .fetch_all(ctx.storage.pool())
        .await?;

        if base_runs.is_empty() || current_runs.is_empty() {
            continue;
        }

        let base_pass = base_runs.iter().filter(|(s, _)| *s == 1).count() as f64
            / base_runs.len() as f64
            * 100.0;
        let curr_pass = current_runs.iter().filter(|(s, _)| *s == 1).count() as f64
            / current_runs.len() as f64
            * 100.0;

        let base_turns: f64 =
            base_runs.iter().map(|(_, t)| *t as f64).sum::<f64>() / base_runs.len() as f64;
        let curr_turns: f64 =
            current_runs.iter().map(|(_, t)| *t as f64).sum::<f64>() / current_runs.len() as f64;

        let pass_delta = curr_pass - base_pass;
        let turns_delta = curr_turns - base_turns;

        // EH.T06 regression thresholds: >5% pass drop or >20% turns increase
        let regression = pass_delta < -5.0 || (base_turns > 0.0 && turns_delta / base_turns > 0.20);
        if regression {
            any_regression = true;
        }

        deltas.push(json!({
            "task_id": tid,
            "description": desc,
            "base_pass_pct": (base_pass * 10.0).round() / 10.0,
            "curr_pass_pct": (curr_pass * 10.0).round() / 10.0,
            "pass_delta": (pass_delta * 10.0).round() / 10.0,
            "base_turns_avg": (base_turns * 10.0).round() / 10.0,
            "curr_turns_avg": (curr_turns * 10.0).round() / 10.0,
            "turns_delta": (turns_delta * 10.0).round() / 10.0,
            "regression": regression,
        }));
    }

    Ok(json!({
        "provider": provider,
        "base_ref": base_ref,
        "deltas": deltas,
        "any_regression": any_regression,
    }))
}

/// bench.list — list all benchmark tasks.
pub async fn list(ctx: &AppContext, _params: Value) -> Result<Value> {
    let rows: Vec<(String, String, String)> = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, description, task_prompt FROM benchmark_tasks ORDER BY id",
    )
    .fetch_all(ctx.storage.pool())
    .await?;

    Ok(json!({
        "tasks": rows.iter().map(|(id, desc, prompt)| json!({
            "id": id,
            "description": desc,
            "prompt_preview": prompt.chars().take(120).collect::<String>(),
        })).collect::<Vec<_>>()
    }))
}

/// bench.seedTasks — insert the default seed benchmark tasks (EH.T05).
pub async fn seed_tasks(ctx: &AppContext, _params: Value) -> Result<Value> {
    let seeds: &[(&str, &str, &str)] = &[
        (
            "BT.001",
            "Fix a compile error (missing semicolon)",
            "The file src/main.rs has a compile error: missing semicolon on line 10. Fix it.",
        ),
        (
            "BT.002",
            "Add a failing unit test fix",
            "src/lib.rs has a function `add(a, b)` that returns `a - b`. Fix the bug so the existing test passes.",
        ),
        (
            "BT.003",
            "Refactor: extract a helper function",
            "In src/utils.rs, the function `process_items` has a 30-line inner loop. Extract it to a helper `process_single_item`.",
        ),
        (
            "BT.004",
            "Add error handling to an unwrap",
            "Replace `file.read_to_string(&mut buf).unwrap()` in src/io.rs with proper error propagation using `?`.",
        ),
        (
            "BT.005",
            "Write a new function from spec",
            "Implement `fn fibonacci(n: u64) -> u64` in src/math.rs. Must pass the three existing tests.",
        ),
    ];

    let mut created = 0u64;
    let mut skipped = 0u64;

    for (id, desc, prompt) in seeds {
        let rows = sqlx::query(
            "INSERT OR IGNORE INTO benchmark_tasks (id, description, task_prompt, success_criteria_json)
             VALUES (?, ?, ?, '[]')",
        )
        .bind(id)
        .bind(desc)
        .bind(prompt)
        .execute(ctx.storage.pool())
        .await?;

        if rows.rows_affected() > 0 {
            created += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(json!({
        "created": created,
        "skipped": skipped,
    }))
}
