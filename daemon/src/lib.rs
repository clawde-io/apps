pub mod account;
pub mod config;
pub mod identity;
pub mod ipc;
pub mod license;
pub mod relay;
pub mod repo;
pub mod service;
pub mod session;
pub mod storage;
pub mod telemetry;
pub mod update;

// Re-export auth so main.rs can use clawd::auth directly.
pub use ipc::auth;

use std::sync::Arc;

use account::AccountRegistry;
use config::DaemonConfig;
use ipc::event::EventBroadcaster;
use license::LicenseInfo;
use repo::RepoRegistry;
use session::SessionManager;
use storage::Storage;
use telemetry::TelemetrySender;
use update::Updater;

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
}
