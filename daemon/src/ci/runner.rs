//! Sprint EE CI.1 — ClawDE CI runner.
//!
//! Executes `.claw/ci.yaml` steps, streaming progress as push events.
//! Designed to run in non-interactive mode (e.g. GitHub Actions, local CI).

use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

use super::config::{CiConfig, CiStep};

/// Status of a CI run.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CiRunStatus {
    Running,
    Success,
    Failure,
    Canceled,
}

impl CiRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Canceled => "canceled",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Canceled => 2,
            _ => 1,
        }
    }

    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::Running)
    }
}

/// Result of a single CI step.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StepResult {
    pub step_index: usize,
    pub step_name: String,
    pub status: String,
    pub output: String,
    pub duration_ms: u64,
}

/// A CI run context.
pub struct CiRun {
    pub run_id: String,
    pub config: CiConfig,
    pub repo_path: String,
    pub step_results: Vec<StepResult>,
    pub status: CiRunStatus,
}

impl CiRun {
    pub fn new(config: CiConfig, repo_path: String) -> Self {
        Self {
            run_id: Uuid::new_v4().to_string(),
            config,
            repo_path,
            step_results: Vec::new(),
            status: CiRunStatus::Running,
        }
    }

    /// Execute all steps, broadcasting events via the provided broadcast fn.
    pub async fn execute<F>(&mut self, broadcast: F) -> Result<CiRunStatus>
    where
        F: Fn(&str, Value) + Send + Sync,
    {
        // Build steps list: either from `steps` array or synthesize from `task`
        let steps = self.build_steps();

        let total = steps.len();
        let run_id = self.run_id.clone();

        for (i, step) in steps.iter().enumerate() {
            let start = std::time::Instant::now();

            broadcast(
                "ci.stepStarted",
                json!({
                    "runId": run_id,
                    "stepIndex": i,
                    "stepName": step.name,
                    "totalSteps": total,
                }),
            );

            let result = self.run_step(step).await;
            let duration_ms = start.elapsed().as_millis() as u64;

            let (status_str, output, succeeded) = match result {
                Ok(out) => ("success", out, true),
                Err(e) => ("failure", e.to_string(), false),
            };

            self.step_results.push(StepResult {
                step_index: i,
                step_name: step.name.clone(),
                status: status_str.to_string(),
                output: output.clone(),
                duration_ms,
            });

            broadcast(
                "ci.stepResult",
                json!({
                    "runId": run_id,
                    "stepIndex": i,
                    "stepName": step.name,
                    "status": status_str,
                    "output": output,
                    "durationMs": duration_ms,
                }),
            );

            if !succeeded && !step.continue_on_error {
                self.status = CiRunStatus::Failure;
                broadcast(
                    "ci.complete",
                    json!({
                        "runId": run_id,
                        "status": "failure",
                        "stepsRun": i + 1,
                        "totalSteps": total,
                    }),
                );
                return Ok(CiRunStatus::Failure);
            }
        }

        self.status = CiRunStatus::Success;
        broadcast(
            "ci.complete",
            json!({
                "runId": run_id,
                "status": "success",
                "stepsRun": total,
                "totalSteps": total,
            }),
        );

        Ok(CiRunStatus::Success)
    }

    fn build_steps(&self) -> Vec<CiStep> {
        if !self.config.steps.is_empty() {
            return self.config.steps.clone();
        }
        // Synthesize from top-level `task`
        vec![super::config::CiStep {
            name: "AI Task".to_string(),
            task: Some(self.config.task.clone()),
            command: None,
            timeout_s: 300,
            continue_on_error: false,
        }]
    }

    async fn run_step(&self, step: &CiStep) -> Result<String> {
        if let Some(cmd) = &step.command {
            // Shell command step
            return self.run_shell_command(cmd, step.timeout_s).await;
        }
        if let Some(task) = &step.task {
            // AI task step — placeholder: in production, spawns a session
            // For now, return a summary indicating the task would run
            return Ok(format!(
                "AI task queued: '{task}' (repo: {})",
                self.repo_path
            ));
        }
        anyhow::bail!("CI step '{}' has neither task nor command", step.name);
    }

    async fn run_shell_command(&self, cmd: &str, timeout_s: u64) -> Result<String> {
        let timeout = std::time::Duration::from_secs(timeout_s);
        let output = tokio::time::timeout(
            timeout,
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&self.repo_path)
                .output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Command timed out after {timeout_s}s"))??;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(anyhow::anyhow!(
                "Command failed (exit {}): {stderr}",
                output.status.code().unwrap_or(-1)
            ))
        }
    }

    /// Return a summary of the run for CI output.
    pub fn summary(&self) -> Value {
        json!({
            "runId": self.run_id,
            "status": self.status.as_str(),
            "repoPath": self.repo_path,
            "steps": self.step_results,
        })
    }
}
