//! Automation config — loads `[automations]` from `.claw/config.toml`.
//!
//! Each automation entry:
//! ```toml
//! [[automations]]
//! name        = "run-tests-on-complete"
//! trigger     = "session_complete"
//! condition   = ""
//! action      = "run_tests"
//! enabled     = true
//!
//! [automations.action_config]
//! command = "cargo test"
//! ```

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{debug, warn};

use super::engine::{ActionType, Automation, TriggerType};

// ─── Raw TOML types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    automations: Vec<AutomationEntry>,
}

#[derive(Debug, Deserialize)]
struct AutomationEntry {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_true")]
    enabled: bool,
    trigger: String,
    #[serde(default)]
    condition: String,
    action: String,
    #[serde(default)]
    action_config: toml::Value,
}

fn default_true() -> bool {
    true
}

// ─── Loader ────────────────────────────────────────────────────────────────

/// Load user automations from `.claw/config.toml` in `repo_path`.
/// Returns an empty vec if the file or the `[automations]` section is missing.
pub fn load_from_repo(repo_path: &Path) -> Vec<Automation> {
    let config_path = repo_path.join(".claw").join("config.toml");
    if !config_path.exists() {
        return vec![];
    }
    match load_file(&config_path) {
        Ok(automations) => {
            debug!(count = automations.len(), "loaded automations from config");
            automations
        }
        Err(e) => {
            warn!(path = %config_path.display(), "failed to parse automations config: {e}");
            vec![]
        }
    }
}

fn load_file(path: &Path) -> Result<Vec<Automation>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let cfg: ConfigFile =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;

    cfg.automations.into_iter().map(entry_to_automation).collect()
}

fn entry_to_automation(e: AutomationEntry) -> Result<Automation> {
    let trigger = parse_trigger(&e.trigger)?;
    let action = parse_action(&e.action)?;
    let action_config = toml_to_json(e.action_config);

    Ok(Automation {
        name: e.name,
        description: e.description,
        enabled: e.enabled,
        trigger,
        condition: if e.condition.is_empty() {
            None
        } else {
            Some(e.condition)
        },
        action,
        action_config,
        builtin: false,
        last_triggered_at: None,
    })
}

fn parse_trigger(s: &str) -> Result<TriggerType> {
    match s {
        "session_complete" => Ok(TriggerType::SessionComplete),
        "task_done" => Ok(TriggerType::TaskDone),
        "file_saved" => Ok(TriggerType::FileSaved),
        "cron" => Ok(TriggerType::Cron),
        other => anyhow::bail!("unknown trigger type: {other}"),
    }
}

fn parse_action(s: &str) -> Result<ActionType> {
    match s {
        "run_tests" => Ok(ActionType::RunTests),
        "send_notification" => Ok(ActionType::SendNotification),
        "create_task" => Ok(ActionType::CreateTask),
        "run_script" => Ok(ActionType::RunScript),
        other => anyhow::bail!("unknown action type: {other}"),
    }
}

fn toml_to_json(v: toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(n) => serde_json::json!(n),
        toml::Value::Float(f) => serde_json::json!(f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_to_json).collect())
        }
        toml::Value::Table(tbl) => {
            serde_json::Value::Object(tbl.into_iter().map(|(k, v)| (k, toml_to_json(v))).collect())
        }
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
    }
}
