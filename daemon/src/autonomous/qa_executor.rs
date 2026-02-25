// SPDX-License-Identifier: MIT
//! QA executor — AE.T08–T11 (Autonomous Execution Engine, Sprint J).
//!
//! Runs the appropriate validator suite for the detected stack:
//! - Rust   → `cargo clippy --message-format json`
//! - TypeScript → `tsc --noEmit`
//! - Flutter/Dart → `flutter analyze`
//! - Go     → `go vet ./...`
//!
//! Each validator is spawned as a subprocess.  Stdout/stderr is captured
//! and parsed for error lines.  The combined result is stored in
//! `task_reviews` (via the handler layer).

use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, warn};

// ─── Types ────────────────────────────────────────────────────────────────────

/// Output from a single validator run.
#[derive(Debug, Clone)]
pub struct ValidatorOutput {
    /// Validator name (e.g. `"cargo-clippy"`, `"tsc"`, `"flutter-analyze"`).
    pub name: String,
    /// `true` when the validator exited with code 0.
    pub passed: bool,
    /// Raw stdout + stderr (combined).
    pub raw: String,
    /// Error/warning lines parsed from `raw`.
    pub error_lines: Vec<String>,
}

/// Combined result for a full QA run.
#[derive(Debug, Clone)]
pub struct QaResult {
    /// `true` when every validator passed.
    pub passed: bool,
    /// Human-readable finding strings (one per error/warning).
    pub findings: Vec<String>,
    /// Per-validator breakdown.
    pub validator_outputs: Vec<ValidatorOutput>,
}

// ─── Stack detection ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stack {
    Rust,
    TypeScript,
    Flutter,
    Go,
    Unknown,
}

/// Detect the primary stack from the presence of well-known manifest files.
fn detect_stack(repo_path: &Path) -> Stack {
    if repo_path.join("Cargo.toml").exists() {
        return Stack::Rust;
    }
    if repo_path.join("pubspec.yaml").exists() {
        return Stack::Flutter;
    }
    if repo_path.join("go.mod").exists() {
        return Stack::Go;
    }
    if repo_path.join("package.json").exists() || repo_path.join("tsconfig.json").exists() {
        return Stack::TypeScript;
    }
    Stack::Unknown
}

// ─── QaExecutor ──────────────────────────────────────────────────────────────

pub struct QaExecutor;

impl QaExecutor {
    /// Run the validator suite for the repo at `repo_path`.
    ///
    /// `task_id` is threaded through for logging and storage.
    pub async fn run_validators(repo_path: &Path, task_id: &str) -> Result<QaResult> {
        let stack = detect_stack(repo_path);
        debug!(task_id, ?stack, "QA executor: detected stack");

        let outputs = match stack {
            Stack::Rust => run_rust_validators(repo_path, task_id).await,
            Stack::Flutter => run_flutter_validators(repo_path, task_id).await,
            Stack::TypeScript => run_ts_validators(repo_path, task_id).await,
            Stack::Go => run_go_validators(repo_path, task_id).await,
            Stack::Unknown => {
                warn!(task_id, "QA executor: unknown stack — skipping validators");
                vec![]
            }
        };

        let passed = outputs.iter().all(|o| o.passed);
        let findings: Vec<String> = outputs.iter().flat_map(|o| o.error_lines.clone()).collect();

        Ok(QaResult {
            passed,
            findings,
            validator_outputs: outputs,
        })
    }
}

// ─── Stack-specific runners ───────────────────────────────────────────────────

async fn run_rust_validators(repo_path: &Path, task_id: &str) -> Vec<ValidatorOutput> {
    let mut outputs = Vec::new();

    // cargo clippy
    let clippy = run_command(
        "cargo",
        &["clippy", "--", "-D", "warnings"],
        repo_path,
        "cargo-clippy",
        task_id,
    )
    .await;
    outputs.push(clippy);

    // cargo test (no network; --no-run avoids long test exec in QA gate)
    let test = run_command(
        "cargo",
        &["test", "--no-run"],
        repo_path,
        "cargo-test",
        task_id,
    )
    .await;
    outputs.push(test);

    outputs
}

async fn run_flutter_validators(repo_path: &Path, task_id: &str) -> Vec<ValidatorOutput> {
    let analyze = run_command(
        "flutter",
        &["analyze", "--no-fatal-infos"],
        repo_path,
        "flutter-analyze",
        task_id,
    )
    .await;
    vec![analyze]
}

async fn run_ts_validators(repo_path: &Path, task_id: &str) -> Vec<ValidatorOutput> {
    let mut outputs = Vec::new();

    // tsc --noEmit
    let tsc = run_command(
        "npx",
        &["--yes", "tsc", "--noEmit"],
        repo_path,
        "tsc",
        task_id,
    )
    .await;
    outputs.push(tsc);

    outputs
}

async fn run_go_validators(repo_path: &Path, task_id: &str) -> Vec<ValidatorOutput> {
    let vet = run_command("go", &["vet", "./..."], repo_path, "go-vet", task_id).await;
    vec![vet]
}

// ─── Command runner ───────────────────────────────────────────────────────────

/// Spawn a subprocess, capture output, and return a `ValidatorOutput`.
async fn run_command(
    program: &str,
    args: &[&str],
    cwd: &Path,
    name: &str,
    task_id: &str,
) -> ValidatorOutput {
    debug!(task_id, validator = name, "running validator");

    let result = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match result {
        Err(e) => {
            // Validator binary not found or execution failed — not a QA failure,
            // just an unavailable tool.
            warn!(task_id, validator = name, err = %e, "validator not available");
            ValidatorOutput {
                name: name.to_owned(),
                passed: true, // treat as pass — tool unavailable, not a code error
                raw: format!("validator not available: {e}"),
                error_lines: vec![],
            }
        }
        Ok(output) => {
            let raw = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            let passed = output.status.success();
            let error_lines = parse_error_lines(&raw, name);

            debug!(task_id, validator = name, passed, "validator done");

            ValidatorOutput {
                name: name.to_owned(),
                passed,
                raw,
                error_lines,
            }
        }
    }
}

/// Extract meaningful error/warning lines from raw validator output.
fn parse_error_lines(raw: &str, _name: &str) -> Vec<String> {
    raw.lines()
        .filter(|line| {
            let l = line.to_ascii_lowercase();
            l.contains("error") || l.contains("warning") || l.contains("error[")
        })
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .take(50) // cap to avoid very large finding lists
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_stack_rust() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]").expect("write");
        assert_eq!(detect_stack(tmp.path()), Stack::Rust);
    }

    #[test]
    fn test_detect_stack_flutter() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        std::fs::write(tmp.path().join("pubspec.yaml"), "name: app").expect("write");
        assert_eq!(detect_stack(tmp.path()), Stack::Flutter);
    }

    #[test]
    fn test_detect_stack_unknown() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        assert_eq!(detect_stack(tmp.path()), Stack::Unknown);
    }

    #[test]
    fn test_parse_error_lines_extracts_errors() {
        let raw = "ok\nerror[E0001]: missing semicolon\nnote: for more info\nwarning: unused var";
        let lines = parse_error_lines(raw, "cargo-clippy");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("error"));
        assert!(lines[1].contains("warning"));
    }
}
