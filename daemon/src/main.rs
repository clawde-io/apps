use anyhow::Result;
use clawd::{
    config::DaemonConfig,
    ipc::event::EventBroadcaster,
    repo::RepoRegistry,
    session::SessionManager,
    storage::Storage,
    AppContext,
};
use clap::Parser;
use std::sync::Arc;
use tracing::info;

#[derive(Parser)]
#[command(
    name = "clawd",
    about = "ClawDE Host — always-on background daemon",
    version
)]
struct Args {
    /// JSON-RPC WebSocket server port
    #[arg(long, default_value_t = 4300, env = "CLAWD_PORT")]
    port: u16,

    /// Data directory for sessions, config, and SQLite database
    #[arg(long, env = "CLAWD_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "CLAWD_LOG")]
    log: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(&args.log)
        .compact()
        .init();

    info!(version = env!("CARGO_PKG_VERSION"), port = args.port, "clawd starting");

    let config = Arc::new(DaemonConfig::new(args.port, args.data_dir, args.log));
    info!(data_dir = %config.data_dir.display(), "data directory");

    let storage = Arc::new(Storage::new(&config.data_dir).await?);
    let broadcaster = Arc::new(EventBroadcaster::new());
    let repo_registry = Arc::new(RepoRegistry::new(broadcaster.clone()));
    let session_manager = Arc::new(SessionManager::new(
        storage.clone(),
        broadcaster.clone(),
        config.data_dir.clone(),
    ));

    let ctx = Arc::new(AppContext {
        config,
        storage,
        broadcaster,
        repo_registry,
        session_manager,
        started_at: std::time::Instant::now(),
    });

    clawd::ipc::run(ctx).await
}
