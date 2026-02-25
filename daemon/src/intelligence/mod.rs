/// Model Intelligence — automatic model selection, context optimization, and cost tracking.
///
/// This module intercepts every outgoing AI message and:
/// 1. Classifies task complexity (Stage 0 in pre-send pipeline)
/// 2. Selects the optimal model for that complexity (Stage 0b)
/// 3. Evaluates response quality and auto-upgrades on failure (post-response hook)
/// 4. Tracks token spend and enforces budget caps (MI.T05+)
/// 5. Optimizes context window to minimize cost (MI.T07+)

pub mod classifier;
pub mod model_router;
pub mod upgrade;

// Stub modules for future phases (MI.T05-T09)
pub mod token_tracker;
pub mod cost;
pub mod context;
pub mod file_context;
pub mod repo_context;

/// Output produced by an AI provider runner.
///
/// The intelligence layer inspects this to decide whether auto-upgrade is warranted.
#[derive(Debug, Clone)]
pub struct RunnerOutput {
    /// The full text content returned by the model.
    pub content: String,
    /// True if the model returned a tool call error (tool not found, schema invalid, etc.).
    pub tool_call_error: bool,
    /// True if the output appears truncated (ends mid-sentence or at max token boundary).
    pub output_truncated: bool,
    /// The model ID that produced this output (e.g. "claude-haiku-4-5").
    pub model_id: String,
    /// Input tokens consumed (0 = unknown).
    pub input_tokens: u64,
    /// Output tokens consumed (0 = unknown).
    pub output_tokens: u64,
}
