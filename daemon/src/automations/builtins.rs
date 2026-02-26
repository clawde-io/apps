//! Built-in automations — always registered, can be disabled but not deleted.
//!
//! Built-ins:
//!  1. `run-tests-on-complete` — runs the project test command after a session ends.
//!  2. `todo-extractor`        — creates a task for every TODO: found in session output.
//!  3. `long-session-notifier` — pushes a notification when a session runs >5 min.

use anyhow::{bail, Result};
use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use crate::AppContext;

use super::engine::{ActionType, Automation, TriggerEvent, TriggerType};

// ─── Built-in definitions ─────────────────────────────────────────────────

pub fn all() -> Vec<Automation> {
    vec![
        Automation {
            name: "run-tests-on-complete".into(),
            description: "Run the project test command after each session completes.".into(),
            enabled: false, // opt-in — user must enable in UI or config
            trigger: TriggerType::SessionComplete,
            condition: None,
            action: ActionType::RunTests,
            action_config: serde_json::json!({ "command": "cargo test" }),
            builtin: true,
            last_triggered_at: None,
        },
        Automation {
            name: "todo-extractor".into(),
            description: "Create a follow-up task for every TODO: found in session output.".into(),
            enabled: true,
            trigger: TriggerType::SessionComplete,
            condition: None,
            action: ActionType::CreateTask,
            action_config: serde_json::json!({ "title_prefix": "TODO from session" }),
            builtin: true,
            last_triggered_at: None,
        },
        Automation {
            name: "long-session-notifier".into(),
            description: "Send a notification when a session runs longer than 5 minutes.".into(),
            enabled: true,
            trigger: TriggerType::SessionComplete,
            condition: Some("session_duration_secs>300".into()),
            action: ActionType::SendNotification,
            action_config: serde_json::json!({ "message": "Your long session has completed." }),
            builtin: true,
            last_triggered_at: None,
        },
    ]
}

// ─── Executor ─────────────────────────────────────────────────────────────

/// Execute a single automation's action for a trigger event.
pub async fn execute(
    automation: &Automation,
    event: &TriggerEvent,
    ctx: &AppContext,
) -> Result<()> {
    info!(name = %automation.name, action = ?automation.action, "executing automation");

    match automation.action {
        ActionType::RunTests => run_tests(automation, ctx).await,
        ActionType::SendNotification => send_notification(automation, event, ctx).await,
        ActionType::CreateTask => create_task_from_event(automation, event, ctx).await,
        ActionType::RunScript => run_script(automation, ctx).await,
    }
}

// ─── Action implementations ────────────────────────────────────────────────

async fn run_tests(automation: &Automation, ctx: &AppContext) -> Result<()> {
    let command = automation
        .action_config
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("cargo test");

    info!("automation run-tests: {command}");
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        bail!("empty test command");
    }
    let output = tokio::process::Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .await?;

    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    ctx.broadcaster.broadcast(
        "automation.testResults",
        serde_json::json!({
            "success": success,
            "stdout": stdout,
            "stderr": stderr,
            "command": command,
        }),
    );

    Ok(())
}

async fn send_notification(
    automation: &Automation,
    event: &TriggerEvent,
    ctx: &AppContext,
) -> Result<()> {
    let message = automation
        .action_config
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Automation triggered.");

    ctx.broadcaster.broadcast(
        "automation.notification",
        serde_json::json!({
            "automationName": automation.name,
            "message": message,
            "sessionId": event.session_id,
            "timestamp": Utc::now().to_rfc3339(),
        }),
    );

    Ok(())
}

async fn create_task_from_event(
    automation: &Automation,
    event: &TriggerEvent,
    ctx: &AppContext,
) -> Result<()> {
    let prefix = automation
        .action_config
        .get("title_prefix")
        .and_then(|v| v.as_str())
        .unwrap_or("Task from automation");

    let output = match &event.session_output {
        Some(o) => o.clone(),
        None => return Ok(()), // nothing to extract
    };

    // Extract TODO: lines from session output.
    let todos: Vec<String> = output
        .lines()
        .filter(|line| {
            let upper = line.to_uppercase();
            upper.contains("TODO:") || upper.contains("TODO :")
        })
        .map(|line| {
            // Strip leading `TODO:` and trim whitespace.
            let trimmed = line.trim();
            if let Some(pos) = trimmed.to_uppercase().find("TODO:") {
                trimmed[pos + 5..].trim().to_string()
            } else {
                trimmed.to_string()
            }
        })
        .filter(|s| !s.is_empty())
        .take(10) // cap at 10 per session
        .collect();

    for todo_text in todos {
        let title = format!("{prefix}: {todo_text}");
        let task_id = format!("todo-{}", Uuid::new_v4());
        info!("automation todo-extractor: creating task '{title}' id={task_id}");
        ctx.task_storage
            .add_task(
                &task_id, &title,
                Some("todo"), None, None, None, None, None, None, None, None, None,
                "",
            )
            .await?;
    }

    Ok(())
}

async fn run_script(automation: &Automation, _ctx: &AppContext) -> Result<()> {
    let script = automation
        .action_config
        .get("script")
        .or_else(|| automation.action_config.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("echo 'no script configured'");

    info!("automation run-script: {script}");
    tokio::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .output()
        .await?;

    Ok(())
}
