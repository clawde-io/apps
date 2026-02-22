use anyhow::Result;
use clap::{Parser, Subcommand};
use clawd::{
    account::AccountRegistry, auth, config::DaemonConfig, identity, ipc::event::EventBroadcaster,
    license, mdns, relay, repo::RepoRegistry, service, session::SessionManager, storage::Storage,
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

    /// JSON-RPC WebSocket server port
    #[arg(long, env = "CLAWD_PORT")]
    port: Option<u16>,

    /// Data directory for sessions, config, and SQLite database
    #[arg(long, env = "CLAWD_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "CLAWD_LOG")]
    log: Option<String>,

    /// Maximum concurrent sessions (0 = unlimited)
    #[arg(long, env = "CLAWD_MAX_SESSIONS")]
    max_sessions: Option<usize>,

    /// Write logs to this file path (rotated daily). Optional.
    #[arg(long, env = "CLAWD_LOG_FILE")]
    log_file: Option<std::path::PathBuf>,
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

    // ── Logging setup ────────────────────────────────────────────────────────
    // Init once — must happen before any tracing calls.
    let log_level = args.log.as_deref().unwrap_or("info").to_owned();
    let _file_guard = setup_logging(&log_level, args.log_file.as_deref());

    match args.command {
        Some(Command::Service { action }) => match action {
            ServiceAction::Install => service::install()?,
            ServiceAction::Uninstall => service::uninstall()?,
            ServiceAction::Status => service::status()?,
        },
        None | Some(Command::Serve) => {
            run_server(args.port, args.data_dir, args.log, args.max_sessions).await?;
        }
    }

    Ok(())
}

/// Initialize the tracing subscriber.
/// If `log_file` is set, logs go to both stdout and a daily-rolling file.
/// Returns a `WorkerGuard` that must stay alive for the process lifetime.
fn setup_logging(
    log_level: &str,
    log_file: Option<&std::path::Path>,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    if let Some(path) = log_file {
        let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let filename = path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("clawd.log"));
        let appender = tracing_appender::rolling::daily(dir, filename);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);

        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(log_level))
            .with(tracing_subscriber::fmt::layer().compact())
            .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
            .init();

        Some(guard)
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(log_level)
            .compact()
            .init();
        None
    }
}

async fn run_server(
    port: Option<u16>,
    data_dir: Option<std::path::PathBuf>,
    log: Option<String>,
    max_sessions: Option<usize>,
) -> Result<()> {
    info!(version = env!("CARGO_PKG_VERSION"), "clawd starting");

    let config = Arc::new(DaemonConfig::new(port, data_dir, log, max_sessions));
    info!(
        data_dir = %config.data_dir.display(),
        port = config.port,
        max_sessions = config.max_sessions,
        "config loaded"
    );

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
        info!(
            count = recovered,
            "recovered stale sessions from previous run"
        );
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
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
            interval.tick().await;
            loop {
                interval.tick().await;
                let info = license::verify_and_cache(&storage, &config, &daemon_id).await;
                *license.write().await = info;
            }
        });
    }

    // ── DB pruning + vacuum (daily, offset 1 h to stagger with license check) ─
    {
        let storage = storage.clone();
        let prune_days = config.session_prune_days;
        tokio::spawn(async move {
            // First run after 1 hour, then every 24 hours
            tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
            loop {
                match storage.prune_old_sessions(prune_days).await {
                    Ok(n) if n > 0 => info!(pruned = n, days = prune_days, "pruned old sessions"),
                    Ok(_) => {}
                    Err(e) => warn!(err = %e, "session pruning failed"),
                }
                if let Err(e) = storage.vacuum().await {
                    warn!(err = %e, "sqlite vacuum failed");
                }
                tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)).await;
            }
        });
    }

    let telemetry = Arc::new(telemetry::spawn(config.clone(), daemon_id.clone(), tier));

    let account_registry = Arc::new(AccountRegistry::new(storage.clone(), broadcaster.clone()));
    let updater = Arc::new(update::spawn(config.clone(), broadcaster.clone()));

    let auth_token = match auth::get_or_create_token(&config.data_dir) {
        Ok(t) => {
            info!("auth token ready");
            t
        }
        Err(e) => {
            warn!("failed to generate auth token: {e:#}; proceeding without auth");
            String::new()
        }
    };

    let ctx = Arc::new(AppContext {
        config: config.clone(),
        storage,
        broadcaster: broadcaster.clone(),
        repo_registry,
        session_manager,
        daemon_id: daemon_id.clone(),
        license: license.clone(),
        telemetry,
        account_registry,
        updater,
        auth_token,
        started_at: std::time::Instant::now(),
    });

    // ── mDNS advertisement ────────────────────────────────────────────────────
    // Non-blocking: if mDNS fails (e.g. system restriction), daemon continues.
    let _mdns_guard = mdns::advertise(&daemon_id, config.port);

    // Spawn relay AFTER ctx is built so it can dispatch inbound RPC frames
    // through the full IPC handler and forward push events to remote clients.
    {
        let lic = license.read().await;
        relay::spawn_if_enabled(config, &lic, daemon_id, ctx.clone()).await;
    }

    clawd::ipc::run(ctx).await
}
