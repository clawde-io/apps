//! Span context for distributed tracing — trace/span IDs and parent linkage.

use uuid::Uuid;

// ─── SpanContext ──────────────────────────────────────────────────────────────

/// Identifies a single operation within a trace hierarchy.
#[derive(Debug, Clone)]
pub struct SpanContext {
    /// UUID shared by all spans in the same task lifecycle.
    pub trace_id: String,
    /// UUID unique to this operation.
    pub span_id: String,
    /// UUID of the enclosing parent operation, if any.
    pub parent_span_id: Option<String>,
}

impl SpanContext {
    /// Start a brand-new trace (no parent, fresh trace_id).
    pub fn root() -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: None,
        }
    }

    /// Create a child span that inherits the trace_id and records this span as parent.
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: Some(self.span_id.clone()),
        }
    }
}

// ─── Span ─────────────────────────────────────────────────────────────────────

/// A timed span.  Records wall-clock start time; caller reads `elapsed_ms()` on
/// completion to get latency.
pub struct Span {
    /// Correlation context for this span.
    pub ctx: SpanContext,
    start: std::time::Instant,
}

impl Span {
    /// Begin timing a new span with the given context.
    pub fn start(ctx: SpanContext) -> Self {
        Self {
            ctx,
            start: std::time::Instant::now(),
        }
    }

    /// Elapsed wall-clock time since `start()` in milliseconds.
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}
