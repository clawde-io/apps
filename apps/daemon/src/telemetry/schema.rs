//! Trace event schema — the canonical shape of every trace record written to JSONL.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single structured trace event written to `.claw/telemetry/traces.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Wall-clock timestamp of when the event was emitted.
    pub ts: DateTime<Utc>,
    /// UUID identifying the task lifecycle this event belongs to.
    pub trace_id: String,
    /// UUID identifying the specific operation (span) within the trace.
    pub span_id: String,
    /// Parent span UUID, if this span is nested inside another.
    pub parent_span_id: Option<String>,
    /// Task ID from the task queue, if associated.
    pub task_id: Option<String>,
    /// Agent identifier that produced this event.
    pub agent_id: Option<String>,
    /// Discriminated kind of event.
    pub kind: TraceKind,
    /// Name of the tool invoked (for ToolCall kind events).
    pub tool: Option<String>,
    /// Wall-clock latency of the operation in milliseconds.
    pub latency_ms: Option<u64>,
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Input tokens consumed (from provider).
    pub tokens_in: Option<u64>,
    /// Output tokens produced (from provider).
    pub tokens_out: Option<u64>,
    /// Estimated cost in USD for this event.
    pub cost_usd: Option<f64>,
    /// Risk flags raised by policy scanners (e.g. "network_access", "write_outside_worktree").
    pub risk_flags: Vec<String>,
    /// True if any field in this event was redacted before writing.
    pub redacted: bool,
}

impl TraceEvent {
    /// Construct a minimal event skeleton; callers fill in remaining fields.
    pub fn new(kind: TraceKind) -> Self {
        use uuid::Uuid;
        Self {
            ts: Utc::now(),
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: None,
            task_id: None,
            agent_id: None,
            kind,
            tool: None,
            latency_ms: None,
            ok: true,
            tokens_in: None,
            tokens_out: None,
            cost_usd: None,
            risk_flags: Vec::new(),
            redacted: false,
        }
    }
}

/// Discriminated union for the kind of operation a trace event represents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceKind {
    /// An agent invoked a tool (Bash, Edit, Read, …).
    ToolCall,
    /// A sub-agent was spawned.
    AgentSpawn,
    /// A task moved from one state to another.
    TaskTransition,
    /// An approval gate was presented to the user.
    ApprovalRequested,
    /// The user granted an approval.
    ApprovalGranted,
    /// The user denied an approval.
    ApprovalDenied,
    /// A request was sent to an AI provider.
    ProviderRequest,
    /// A response was received from an AI provider.
    ProviderResponse,
    /// An error occurred.
    Error,
    /// A periodic checkpoint was recorded.
    Checkpoint,
}
