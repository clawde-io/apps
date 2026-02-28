//! Per-session persistent connection state for AI providers (Sprint BB PV.5-6).
//!
//! The `ProviderSessionRegistry` tracks one `ProviderSession` per ClawDE
//! session ID. For Codex (OpenAI Responses API) sessions, it stores the
//! `previous_response_id` so each turn only sends the new user message,
//! not the full conversation history — achieving server-side caching.
//!
//! For Claude sessions the registry is used for session affinity tracking;
//! the `--resume` flag on the `claude` CLI already handles history chaining.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

use super::capabilities::Provider;

// ─── ProviderSession ──────────────────────────────────────────────────────────

/// Per-ClawDE-session state for a connected AI provider.
#[derive(Debug, Clone)]
pub struct ProviderSession {
    /// The ClawDE session ID this provider session is bound to.
    pub session_id: String,
    /// Which provider this session is connected to.
    pub provider: Provider,
    /// The most recent response ID from the OpenAI Responses API.
    /// Passed as `--previous-response-id` on the next Codex turn so the
    /// server only processes the delta, not the full history.
    pub previous_response_id: Option<String>,
    /// When this session last sent or received a message.
    pub last_active: Instant,
}

impl ProviderSession {
    /// Create a new provider session bound to a ClawDE session.
    pub fn new(session_id: String, provider: Provider) -> Self {
        Self {
            session_id,
            provider,
            previous_response_id: None,
            last_active: Instant::now(),
        }
    }

    /// Record the response ID from the last completed AI turn.
    /// Called after each successful Codex response to enable chaining.
    pub fn chain_response(&mut self, response_id: String) {
        self.previous_response_id = Some(response_id);
        self.last_active = Instant::now();
    }

    /// Update last-active timestamp (called at the start of each turn).
    pub fn touch(&mut self) {
        self.last_active = Instant::now();
    }

    /// True if this session has been idle longer than `idle_timeout`.
    pub fn is_stale(&self, idle_timeout: Duration) -> bool {
        self.last_active.elapsed() > idle_timeout
    }
}

// ─── ProviderSessionRegistry ──────────────────────────────────────────────────

/// In-memory registry of all active provider sessions.
///
/// Keyed by ClawDE session ID. Stale sessions (idle > 5 min) are evicted
/// on the next `get_or_create` call to prevent unbounded memory growth.
pub struct ProviderSessionRegistry {
    sessions: HashMap<String, ProviderSession>,
    /// Sessions idle longer than this are automatically closed.
    idle_timeout: Duration,
}

impl ProviderSessionRegistry {
    /// Create a new registry with a 5-minute idle timeout.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            idle_timeout: Duration::from_secs(300),
        }
    }

    /// Return a mutable reference to the provider session for a ClawDE
    /// session, creating a fresh one if none exists. Evicts stale sessions.
    pub fn get_or_create(&mut self, session_id: &str, provider: Provider) -> &mut ProviderSession {
        self.evict_stale();
        self.sessions
            .entry(session_id.to_string())
            .or_insert_with(|| ProviderSession::new(session_id.to_string(), provider))
    }

    /// Record a new `previous_response_id` for an existing session.
    ///
    /// No-op if the session doesn't exist (will be created on next turn).
    pub fn update_response_id(&mut self, session_id: &str, response_id: String) {
        if let Some(s) = self.sessions.get_mut(session_id) {
            s.chain_response(response_id);
        }
    }

    /// Return the last response ID for chaining, if one exists.
    pub fn previous_response_id(&self, session_id: &str) -> Option<&str> {
        self.sessions
            .get(session_id)
            .and_then(|s| s.previous_response_id.as_deref())
    }

    /// Remove the provider session for a ClawDE session (called on session delete).
    pub fn remove(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    /// Drop all sessions that have been idle longer than `idle_timeout`.
    pub fn evict_stale(&mut self) {
        let timeout = self.idle_timeout;
        self.sessions.retain(|_, s| !s.is_stale(timeout));
    }

    /// Number of currently tracked provider sessions (including stale).
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// True if no sessions are tracked.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Default for ProviderSessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe shared provider session registry.
pub type SharedProviderSessionRegistry = Arc<RwLock<ProviderSessionRegistry>>;

/// Construct a new shared registry.
pub fn new_shared_registry() -> SharedProviderSessionRegistry {
    Arc::new(RwLock::new(ProviderSessionRegistry::new()))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_or_create_returns_same_session() {
        let mut reg = ProviderSessionRegistry::new();
        let _s = reg.get_or_create("sess-1", Provider::Codex);
        assert_eq!(reg.len(), 1);
        let _s2 = reg.get_or_create("sess-1", Provider::Codex);
        assert_eq!(reg.len(), 1);
        let _s3 = reg.get_or_create("sess-2", Provider::Claude);
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn chain_response_stores_id() {
        let mut reg = ProviderSessionRegistry::new();
        reg.get_or_create("sess-1", Provider::Codex);
        reg.update_response_id("sess-1", "resp-abc123".to_string());
        assert_eq!(reg.previous_response_id("sess-1"), Some("resp-abc123"));
    }

    #[test]
    fn remove_clears_session() {
        let mut reg = ProviderSessionRegistry::new();
        reg.get_or_create("sess-1", Provider::Codex);
        reg.remove("sess-1");
        assert!(reg.is_empty());
    }

    #[test]
    fn stale_session_evicted() {
        let mut reg = ProviderSessionRegistry {
            sessions: HashMap::new(),
            idle_timeout: Duration::from_millis(1),
        };
        reg.get_or_create("sess-stale", Provider::Codex);
        std::thread::sleep(Duration::from_millis(5));
        reg.evict_stale();
        assert!(reg.is_empty());
    }

    #[test]
    fn no_response_id_before_chain() {
        let mut reg = ProviderSessionRegistry::new();
        reg.get_or_create("sess-1", Provider::Codex);
        assert!(reg.previous_response_id("sess-1").is_none());
    }
}
