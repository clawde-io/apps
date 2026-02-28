//! Router agent — classifies user requests and decides task routing (Phase 43e).

use serde::{Deserialize, Serialize};

/// Output from the Router agent's classification step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterOutput {
    pub action: RouterAction,
    pub reason: String,
    pub task_titles: Vec<String>,
    pub risk_flags: Vec<String>,
}

/// The action the Router agent recommends the daemon take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterAction {
    CreateTask,
    AppendToThread,
    ShowStatus,
    RequestApproval,
    AnswerQuestion,
}

/// Parse the Router agent's JSON output.
pub fn parse_router_output(json: &str) -> Result<RouterOutput, serde_json::Error> {
    serde_json::from_str(json)
}

/// System prompt content for the Router agent role.
pub fn router_prompt_content() -> &'static str {
    "You are the Router agent for ClawDE. Classify user requests and output \
ONLY valid JSON with fields: action \
(create_task|append_to_thread|show_status|request_approval|answer_question), \
reason (string), task_titles (array of strings), risk_flags (array of strings). \
No prose outside the JSON object."
}
