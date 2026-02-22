use anyhow::Result;
use clap::{Parser, Subcommand};
use clawd::{
    account::AccountRegistry, config::DaemonConfig, identity, ipc::event::EventBroadcaster,
    license, relay, repo::RepoRegistry, service, session::SessionManager, storage::Storage,
    telemetry, update, AppContext,
};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Parser)]
#[command(
    name = "clawd",
    about = "ClawDE Host — always-on background daemon",
    version
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// JSON-RPC WebSocket server port (serve command only)
    #[arg(long, default_value_t = 4300, env = "CLAWD_PORT")]
    port: u16,

    /// Data directory for sessions, config, and SQLite database
    #[arg(long, env = "CLAWD_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "CLAWD_LOG")]
    log: String,
}

#[derive(Subcommand)]
enum Command {
    /// Start the daemon server (default when no subcommand given)
    Serve,
    /// Manage the daemon system service
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install and start clawd as a platform service
    Install,
    /// Stop and remove the platform service
    Uninstall,
    /// Show the service status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(&args.log)
        .compact()
        .init();

    match args.command {
        Some(Command::Service { action }) => match action {
            ServiceAction::Install => service::install()?,
            ServiceAction::Uninstall => service::uninstall()?,
            ServiceAction::Status => service::status()?,
        },
        None | Some(Command::Serve) => {
            run_server(args.port, args.data_dir, args.log).await?;
        }
    }

    Ok(())
}

async fn run_server(
    port: u16,
    data_dir: Option<std::path::PathBuf>,
    log: String,
) -> Result<()> {
    info!(
        version = env!("CARGO_PKG_VERSION"),
        port = port,
        "clawd starting"
    );

    let config = Arc::new(DaemonConfig::new(port, data_dir, log));
    info!(data_dir = %config.data_dir.display(), "data directory");

    let storage = Arc::new(Storage::new(&config.data_dir).await?);

    let daemon_id = match identity::get_or_create(&storage).await {
        Ok(id) => {
            info!(daemon_id = %id, "daemon identity ready");
            id
        }
        Err(e) => {
            warn!("failed to get daemon_id: {e:#}; proceeding without identity");
            String::new()
        }
    };

    let broadcaster = Arc::new(EventBroadcaster::new());
    let repo_registry = Arc::new(RepoRegistry::new(broadcaster.clone()));
    let session_manager = Arc::new(SessionManager::new(
        storage.clone(),
        broadcaster.clone(),
        config.data_dir.clone(),
    ));

    let recovered = storage.recover_stale_sessions().await.unwrap_or(0);
    if recovered > 0 {
        info!(count = recovered, "recovered stale sessions from previous run");
    }

    let license_info = license::verify_and_cache(&storage, &config, &daemon_id).await;
    let tier = license_info.tier.clone();
    let license = Arc::new(tokio::sync::RwLock::new(license_info));

    {
        let storage = storage.clone();
        let config = config.clone();
        let daemon_id = daemon_id.clone();
        let license = license.clone();
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
            interval.tick().await;
            loop {
                interval.tick().await;
                let info = license::verify_and_cache(&storage, &config, &daemon_id).await;
                *license.write().await = info;
            }
        });
    }

    let telemetry = Arc::new(telemetry::spawn(config.clone(), daemon_id.clone(), tier));

    let relay_client = {
        let lic = license.read().await;
        relay::spawn_if_enabled(config.clone(), &lic, daemon_id.clone(), broadcaster.clone()).await
    };

    let account_registry = Arc::new(AccountRegistry::new(storage.clone(), broadcaster.clone()));
    let updater = Arc::new(update::spawn(config.clone(), broadcaster.clone()));

    let ctx = Arc::new(AppContext {
        config,
        storage,
        broadcaster,
        repo_registry,
        session_manager,
        daemon_id,
        license,
        telemetry,
        relay_client,
        account_registry,
        updater,
        started_at: std::time::Instant::now(),
    });

    clawd::ipc::run(ctx).await
}
