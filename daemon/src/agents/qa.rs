//! QA Executor agent — runs test suites and interprets failures (Phase 43e).

use serde::{Deserialize, Serialize};

/// A single test failure record produced by the QA Executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaFailure {
    pub test: String,
    pub output: String,
    pub likely_cause: String,
}

/// QA verdict: whether the task passes, fails, or should be retried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QaVerdict {
    Pass,
    Fail,
    Retry,
}

/// Full output from the QA Executor agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaOutput {
    pub tests_ran: Vec<String>,
    pub tests_passed: u32,
    pub tests_failed: u32,
    pub failures: Vec<QaFailure>,
    pub evidence: String,
    pub verdict: QaVerdict,
}

/// Parse the QA Executor agent's JSON output.
pub fn parse_qa_output(json: &str) -> Result<QaOutput, serde_json::Error> {
    serde_json::from_str(json)
}

/// System prompt content for the QA Executor agent role.
pub fn qa_prompt_content() -> &'static str {
    "You are the QA Executor agent for ClawDE. Run the test suite for the \
task. Interpret failures — distinguish test bugs from implementation bugs. \
Attach evidence (test output). \
Output JSON: { tests_ran: [...], tests_passed: N, tests_failed: N, \
failures: [...], evidence: '...', verdict: 'pass'|'fail'|'retry' }. \
If all tests pass: call transition_task with new_state: 'done'. \
If failing: call transition_task with new_state: 'active'."
}
