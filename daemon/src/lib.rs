pub mod config;
pub mod ipc;
pub mod repo;
pub mod session;
pub mod storage;

use std::sync::Arc;

use config::DaemonConfig;
use ipc::event::EventBroadcaster;
use repo::RepoRegistry;
use session::SessionManager;
use storage::Storage;

/// Shared application state passed to every RPC handler and background task.
#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<DaemonConfig>,
    pub storage: Arc<Storage>,
    pub broadcaster: Arc<EventBroadcaster>,
    pub repo_registry: Arc<RepoRegistry>,
    pub session_manager: Arc<SessionManager>,
    pub started_at: std::time::Instant,
}
