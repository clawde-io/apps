pub mod account;
pub mod agents;
pub mod intelligence;
pub mod claw_init;
pub mod config;
pub mod evals;
pub mod identity;
pub mod ipc;
pub mod license;
pub mod mcp;
pub mod mdns;
pub mod policy;
pub mod relay;
pub mod repo;
pub mod scheduler;
pub mod service;
pub mod session;
pub mod storage;
pub mod tasks;
pub mod telemetry;
pub mod threads;
pub mod update;
pub mod context_manager;
pub mod process_pool;
pub mod resource_governor;
pub mod worktree;
pub mod security;
pub mod observability;
pub mod doctor;
pub mod drift;
pub mod init_templates;
pub mod metrics;
pub mod providers_knowledge;
pub mod standards;
pub mod task_engine;
pub mod cli;
pub mod repo_intelligence;
pub mod session_intelligence;

// Re-export auth so main.rs can use clawd::auth directly.
pub use ipc::auth;

use std::sync::Arc;

use account::AccountRegistry;
use agents::orchestrator::{Orchestrator, SharedOrchestrator};
use config::DaemonConfig;
use intelligence::token_tracker::TokenTracker;
use ipc::event::EventBroadcaster;
use license::LicenseInfo;
use repo::RepoRegistry;
use scheduler::accounts::{AccountPool, SharedAccountPool};
use scheduler::fallback::{FallbackEngine, SharedFallbackEngine};
use scheduler::queue::{SchedulerQueue, SharedSchedulerQueue};
use scheduler::rate_limits::{RateLimitTracker, SharedRateLimitTracker};
use session::SessionManager;
use storage::Storage;
use tasks::TaskStorage;
use telemetry::TelemetrySender;
use doctor::version_watcher::VersionWatcher;
use metrics::SharedMetrics;
use update::Updater;
use worktree::manager::{SharedWorktreeManager, WorktreeManager};

/// Shared application state passed to every RPC handler and background task.
#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<DaemonConfig>,
    pub storage: Arc<Storage>,
    pub broadcaster: Arc<EventBroadcaster>,
    pub repo_registry: Arc<RepoRegistry>,
    pub session_manager: Arc<SessionManager>,
    pub started_at: std::time::Instant,
    /// Stable machine identity (SHA-256 of platform hardware ID).
    pub daemon_id: String,
    /// Current license tier and feature flags.
    pub license: Arc<tokio::sync::RwLock<LicenseInfo>>,
    /// Telemetry event sender (fire-and-forget).
    pub telemetry: Arc<TelemetrySender>,
    /// Multi-account pool manager.
    pub account_registry: Arc<AccountRegistry>,
    /// Self-update manager.
    pub updater: Arc<Updater>,
    /// Local WebSocket auth token.  Every new connection must send a
    /// `daemon.auth` RPC with this token before any other method call.
    /// Empty string means auth is disabled (not recommended).
    pub auth_token: String,
    /// Task queue storage (agent_tasks, activity_log, agent_registry, work_sessions).
    pub task_storage: Arc<TaskStorage>,
    /// Per-task Git worktree manager (Phase 43c).
    pub worktree_manager: SharedWorktreeManager,
    /// In-memory account pool for the scheduler (Phase 43m).
    pub account_pool: SharedAccountPool,
    /// Per-account sliding-window rate-limit tracker (Phase 43m).
    pub rate_limit_tracker: SharedRateLimitTracker,
    /// Provider fallback engine (Phase 43m).
    pub fallback_engine: SharedFallbackEngine,
    /// Priority-ordered scheduling queue (Phase 43m).
    pub scheduler_queue: SharedSchedulerQueue,
    /// Multi-agent orchestrator (Phase 43e).
    pub orchestrator: SharedOrchestrator,
    /// Token usage tracker — records input/output tokens per AI response (Phase 61 MI.T05).
    pub token_tracker: TokenTracker,
    /// In-process Prometheus-style metrics counters (DC.T49).
    pub metrics: SharedMetrics,
    /// Version bump watcher — polls manifest files and fires `warning.versionBump` (D64.T16).
    pub version_watcher: Arc<VersionWatcher>,
    /// In-memory registry of connected IDE extensions and their editor contexts (Sprint Z).
    pub ide_bridge: crate::ide::SharedVsCodeBridge,
}

impl AppContext {
    /// Initialise scheduler and worktree fields with sensible defaults.
    ///
    /// Called after constructing the base `AppContext` to wire together
    /// the Phase 43 components (worktree manager + scheduler).
    pub fn init_scheduler_and_worktrees(mut self, data_dir: &std::path::Path) -> Self {
        let account_pool = Arc::new(AccountPool::new());
        let rate_limit_tracker = Arc::new(RateLimitTracker::new());
        let fallback_engine = Arc::new(FallbackEngine::new(
            Arc::clone(&account_pool),
            Arc::clone(&rate_limit_tracker),
        ));

        self.worktree_manager = Arc::new(WorktreeManager::new(data_dir));
        self.account_pool = account_pool;
        self.rate_limit_tracker = rate_limit_tracker;
        self.fallback_engine = fallback_engine;
        self.scheduler_queue = Arc::new(SchedulerQueue::new());
        self.orchestrator = Arc::new(Orchestrator::new());
        self
    }
}
pub mod pairing;
pub mod project;
pub mod packs;

// Sprint I — Provider Onboarding
pub mod providers_onboarding;

// Sprint J — Autonomous Execution Engine
pub mod autonomous;

// Sprint K — Arena Mode + Code Completion
pub mod arena;
pub mod completion;

// Sprint L — Visual & Multimodal
pub mod browser_tool;

// Sprint N — Multi-Repo Orchestration
pub mod mailbox;
pub mod topology;

// Sprint O — AI Code Review Engine
pub mod code_review;

// Sprint P — Builder Mode
pub mod builder;

// Sprint Q — Analytics
pub mod analytics;

// Sprint S — LSP + VS Code compatibility
pub mod lsp;
pub mod vscode;

// Sprint V — Intelligence & Extensibility
pub mod prompt_intelligence;

// Sprint X — Production Reliability utilities
pub mod health;
pub mod circuit_breaker;
pub mod retry;

// Sprint Z — IDE Extension Host + Performance
pub mod ide;
pub mod perf;
