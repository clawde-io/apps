//! High-level trace emission helpers.
//!
//! `TraceEmitter` wraps a `TracesWriter` and provides named methods for each
//! event kind so call-sites don't have to construct `TraceEvent` by hand.

use std::sync::Arc;

use anyhow::Result;
use uuid::Uuid;

use super::redact::redact_trace;
use super::schema::{TraceEvent, TraceKind};
use super::traces::TracesWriter;

// ─── TraceEmitter ─────────────────────────────────────────────────────────────

/// Emits structured trace events into the JSONL store.
///
/// Each emitter maintains a *current trace ID* that is shared across
/// correlated events within a single task lifecycle.  Call `begin_trace()` to
/// start a new lifecycle and `end_trace()` when it completes.
#[derive(Clone)]
pub struct TraceEmitter {
    writer: Arc<TracesWriter>,
    /// Active trace_id, if a task lifecycle is in progress.
    current_trace_id: Arc<tokio::sync::RwLock<Option<String>>>,
}

impl TraceEmitter {
    /// Create a new emitter backed by the given writer.
    pub fn new(writer: Arc<TracesWriter>) -> Self {
        Self {
            writer,
            current_trace_id: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Start a new trace lifecycle, returning the fresh trace_id.
    pub async fn begin_trace(&self) -> String {
        let id = Uuid::new_v4().to_string();
        *self.current_trace_id.write().await = Some(id.clone());
        id
    }

    /// End the current trace lifecycle.
    pub async fn end_trace(&self) {
        *self.current_trace_id.write().await = None;
    }

    // ─── Event-specific emission methods ─────────────────────────────────────

    /// Emit a tool call span.
    pub async fn tool_call(
        &self,
        task_id: &str,
        agent_id: &str,
        tool: &str,
        latency_ms: u64,
        ok: bool,
        risk_flags: &[String],
    ) -> Result<()> {
        let mut event = self.new_trace_event(TraceKind::ToolCall).await;
        event.task_id = Some(task_id.to_string());
        event.agent_id = Some(agent_id.to_string());
        event.tool = Some(tool.to_string());
        event.latency_ms = Some(latency_ms);
        event.ok = ok;
        event.risk_flags = risk_flags.to_vec();
        self.emit(event).await
    }

    /// Emit an agent spawn event.
    pub async fn agent_spawn(
        &self,
        task_id: &str,
        agent_id: &str,
        role: &str,
    ) -> Result<()> {
        let mut event = self.new_trace_event(TraceKind::AgentSpawn).await;
        event.task_id = Some(task_id.to_string());
        event.agent_id = Some(agent_id.to_string());
        // Store role in tool field for visibility; not strictly a tool invocation.
        event.tool = Some(role.to_string());
        event.ok = true;
        self.emit(event).await
    }

    /// Emit a task state transition.
    pub async fn task_transition(
        &self,
        task_id: &str,
        from: &str,
        to: &str,
    ) -> Result<()> {
        let mut event = self.new_trace_event(TraceKind::TaskTransition).await;
        event.task_id = Some(task_id.to_string());
        // Encode transition as "from→to" in the tool field for easy querying.
        event.tool = Some(format!("{from}→{to}"));
        event.ok = true;
        self.emit(event).await
    }

    /// Emit an approval gate event.
    ///
    /// `kind` should be one of `ApprovalRequested`, `ApprovalGranted`, or
    /// `ApprovalDenied`.
    pub async fn approval(
        &self,
        task_id: &str,
        approval_id: &str,
        kind: TraceKind,
    ) -> Result<()> {
        let mut event = self.new_trace_event(kind).await;
        event.task_id = Some(task_id.to_string());
        event.tool = Some(approval_id.to_string());
        event.ok = true;
        self.emit(event).await
    }

    /// Emit a provider request/response pair.
    ///
    /// Call once when the round-trip completes (not separately for request and
    /// response) to capture both latency and token counts in one record.
    pub async fn provider(
        &self,
        task_id: &str,
        agent_id: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        latency_ms: u64,
        ok: bool,
    ) -> Result<()> {
        use super::cost::estimate_cost_usd;

        let mut event = self.new_trace_event(TraceKind::ProviderResponse).await;
        event.task_id = Some(task_id.to_string());
        event.agent_id = Some(agent_id.to_string());
        event.tool = Some(model.to_string());
        event.tokens_in = Some(tokens_in);
        event.tokens_out = Some(tokens_out);
        event.cost_usd = Some(estimate_cost_usd(tokens_in, tokens_out, model));
        event.latency_ms = Some(latency_ms);
        event.ok = ok;
        self.emit(event).await
    }

    // ─── Private ─────────────────────────────────────────────────────────────

    /// Build a skeleton `TraceEvent`, inheriting the current trace_id.
    async fn new_trace_event(&self, kind: TraceKind) -> TraceEvent {
        let trace_id = self
            .current_trace_id
            .read()
            .await
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut event = TraceEvent::new(kind);
        event.trace_id = trace_id;
        event
    }

    /// Redact, then write.
    async fn emit(&self, mut event: TraceEvent) -> Result<()> {
        redact_trace(&mut event);
        self.writer.write(&event).await
    }
}
