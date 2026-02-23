use anyhow::Result;
use clap::{Parser, Subcommand};
use clawd::{
    account::AccountRegistry, auth, config::DaemonConfig, identity, ipc::event::EventBroadcaster,
    license, mdns, relay, repo::RepoRegistry, service, session::SessionManager, storage::Storage,
    tasks::{
        storage::{ActivityQueryParams, AgentTaskRow, TaskListParams},
        TaskStorage,
    },
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
    /// Scaffold .claude/ directory structure for a project
    Init {
        /// Project path to initialize (default: current directory)
        path: Option<std::path::PathBuf>,
    },
    /// Manage agent tasks
    Tasks {
        #[command(subcommand)]
        action: TasksAction,
    },
}

#[derive(Subcommand)]
enum TasksAction {
    /// List tasks (optionally filter by repo, status, phase)
    List {
        #[arg(long, short)]
        repo: Option<String>,
        #[arg(long, short)]
        status: Option<String>,
        #[arg(long, short = 'p')]
        phase: Option<String>,
        #[arg(long, short = 'n', default_value = "50")]
        limit: i64,
        /// Output as JSON array (for piping)
        #[arg(long)]
        json: bool,
    },
    /// Get full detail of a task by ID
    Get {
        /// Task ID (positional or --task)
        id: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Claim a task atomically (mark in-progress)
    Claim {
        /// Task ID (positional or --task)
        id: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long, default_value = "cli")]
        agent: String,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Release a task back to pending
    Release {
        id: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long, default_value = "cli")]
        agent: String,
    },
    /// Mark a task done (notes required)
    Done {
        /// Task ID (positional or --task)
        id: Option<String>,
        #[arg(long)]
        task: Option<String>,
        /// Completion notes (required — daemon enforces non-empty)
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, default_value = "cli")]
        agent: String,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Mark a task blocked
    Blocked {
        id: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, default_value = "cli")]
        agent: String,
    },
    /// Send a heartbeat for a running task
    Heartbeat {
        id: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long, default_value = "cli")]
        agent: String,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Add a new task
    Add {
        #[arg(long)]
        title: String,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        phase: Option<String>,
        #[arg(long, default_value = "medium")]
        severity: String,
        #[arg(long)]
        file: Option<String>,
    },
    /// Log an activity entry (called by PostToolUse hook)
    Log {
        /// Task ID (positional or --task; optional)
        id: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long, default_value = "cli")]
        agent: String,
        #[arg(long)]
        action: String,
        /// Detail text (alias: --notes)
        #[arg(long)]
        detail: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, default_value = "auto", name = "entry-type")]
        entry_type: String,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Post a narrative note for a task or phase
    Note {
        /// Task ID (positional or --task; omit for phase-level note)
        id: Option<String>,
        #[arg(long, conflicts_with = "phase")]
        task: Option<String>,
        /// Phase name for a phase-level note
        #[arg(long)]
        phase: Option<String>,
        /// Note text (positional or --note)
        text: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, default_value = "cli")]
        agent: String,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Import tasks from a planning markdown file
    FromPlanning {
        /// Path to a planning .md file (e.g. .claude/planning/41-feature.md)
        file: std::path::PathBuf,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Sync active.md → DB and regenerate queue.json
    Sync {
        #[arg(long)]
        repo: Option<String>,
        /// Path to active.md (default: {repo}/.claude/tasks/active.md)
        #[arg(long)]
        active_md: Option<std::path::PathBuf>,
    },
    /// Show task counts summary for a project
    Summary {
        #[arg(long)]
        repo: Option<String>,
        /// Output raw JSON instead of formatted table
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Show recent activity log
    Activity {
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        phase: Option<String>,
        #[arg(long, default_value = "20")]
        limit: i64,
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
        Some(Command::Init { path }) => {
            let path = path.unwrap_or_else(|| std::env::current_dir().unwrap());
            run_init(&path).await?;
        }
        Some(Command::Tasks { action }) => {
            run_tasks(action, args.data_dir).await?;
        }
        None | Some(Command::Serve) => {
            run_server(args.port, args.data_dir, args.log, args.max_sessions).await?;
        }
    }

    Ok(())
}

/// Initialize the tracing subscriber.
/// If `log_file` is set, logs go to both stdout and a daily-rolling file.
/// Returns a `WorkerGuard` that must stay alive for the process lifetime.
///
/// If the log directory cannot be created, falls back to stdout-only logging
/// with a warning — never panics.
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

        // Ensure the directory exists before tracing-appender tries to open it.
        if let Err(e) = std::fs::create_dir_all(dir) {
            // Fall back to stdout-only — don't panic on a bad log path.
            eprintln!(
                "warn: could not create log directory '{}': {e} — falling back to stdout",
                dir.display()
            );
            tracing_subscriber::fmt()
                .with_env_filter(log_level)
                .compact()
                .init();
            return None;
        }

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

// ── clawd init ────────────────────────────────────────────────────────────────

async fn run_init(path: &std::path::Path) -> Result<()> {
    use tokio::fs;

    let claude_dir = path.join(".claude");
    let mut created: Vec<String> = Vec::new();

    for dir in &[
        ".claude",
        ".claude/rules",
        ".claude/agents",
        ".claude/skills",
        ".claude/memory",
        ".claude/tasks",
        ".claude/planning",
        ".claude/qa",
        ".claude/docs",
        ".claude/archive/inbox",
        ".claude/inbox",
        ".claude/temp",
    ] {
        let full = path.join(dir);
        if !full.exists() {
            fs::create_dir_all(&full).await?;
            created.push(dir.to_string());
        }
    }

    let claude_md = claude_dir.join("CLAUDE.md");
    if !claude_md.exists() {
        fs::write(&claude_md, clawd::ipc::handlers::afs::CLAUDE_MD_TEMPLATE).await?;
        created.push(".claude/CLAUDE.md".to_string());
    }

    let active_md = claude_dir.join("tasks/active.md");
    if !active_md.exists() {
        fs::write(&active_md, clawd::ipc::handlers::afs::ACTIVE_MD_TEMPLATE).await?;
        created.push(".claude/tasks/active.md".to_string());
    }

    let settings = claude_dir.join("settings.json");
    if !settings.exists() {
        fs::write(&settings, clawd::ipc::handlers::afs::SETTINGS_JSON_TEMPLATE).await?;
        created.push(".claude/settings.json".to_string());
    }

    // Ensure .claude/ is in .gitignore
    let gitignore = path.join(".gitignore");
    if gitignore.exists() {
        let content = fs::read_to_string(&gitignore).await.unwrap_or_default();
        if !content.contains(".claude/") {
            let updated = format!("{}\n# AI agent directories\n.claude/\n", content.trim_end());
            fs::write(&gitignore, updated).await?;
        }
    } else {
        fs::write(&gitignore, ".claude/\n").await?;
        created.push(".gitignore".to_string());
    }

    if created.is_empty() {
        println!("Already initialized: {}", path.display());
    } else {
        println!("Initialized AFS at: {}", path.display());
        for item in &created {
            println!("  created  {item}");
        }
    }
    Ok(())
}

// ── clawd tasks ───────────────────────────────────────────────────────────────

/// Open the task DB for CLI commands (no server — just storage access).
async fn open_task_storage(data_dir: Option<std::path::PathBuf>) -> Result<TaskStorage> {
    let config = DaemonConfig::new(None, data_dir, Some("error".to_string()), None);
    let storage = Storage::new(&config.data_dir).await?;
    Ok(TaskStorage::new(storage.pool().clone()))
}

/// Resolve task ID from positional arg or --task flag.
fn resolve_task_id(id: Option<String>, task: Option<String>) -> Result<String> {
    id.or(task).ok_or_else(|| anyhow::anyhow!("task ID required (positional or --task)"))
}

async fn run_tasks(action: TasksAction, data_dir: Option<std::path::PathBuf>) -> Result<()> {
    let ts = open_task_storage(data_dir).await?;

    match action {
        TasksAction::List { repo, status, phase, limit, json } => {
            let tasks = ts.list_tasks(&TaskListParams {
                repo_path: repo,
                status,
                phase,
                limit: Some(limit),
                ..Default::default()
            }).await?;
            if json {
                println!("{}", serde_json::to_string(&tasks)?);
            } else if tasks.is_empty() {
                println!("No tasks found.");
            } else {
                println!("{:<12} {:<10} {:<10} {}", "STATUS", "SEVERITY", "PHASE", "TITLE");
                println!("{}", "-".repeat(72));
                for t in &tasks {
                    println!(
                        "{:<12} {:<10} {:<10} {}",
                        t.status,
                        t.severity.as_deref().unwrap_or("-"),
                        t.phase.as_deref().unwrap_or("-"),
                        t.title
                    );
                }
                println!("\n{} task(s)", tasks.len());
            }
        }

        TasksAction::Get { id, task, .. } => {
            let task_id = resolve_task_id(id, task)?;
            match ts.get_task(&task_id).await? {
                None => {
                    eprintln!("Task not found: {task_id}");
                    std::process::exit(1);
                }
                Some(t) => print_task_detail(&t),
            }
        }

        TasksAction::Claim { id, task, agent, .. } => {
            let task_id = resolve_task_id(id, task)?;
            let t = ts.claim_task(&task_id, &agent).await?;
            println!("Claimed: {} — {}", t.id, t.title);
            println!("Status: {} by {}", t.status, t.claimed_by.as_deref().unwrap_or("?"));
        }

        TasksAction::Release { id, task, agent } => {
            let task_id = resolve_task_id(id, task)?;
            ts.release_task(&task_id, &agent).await?;
            println!("Released: {task_id}");
        }

        TasksAction::Done { id, task, notes, agent: _, .. } => {
            let task_id = resolve_task_id(id, task)?;
            let notes_text = notes.ok_or_else(|| anyhow::anyhow!("--notes required for done"))?;
            let t = ts.update_status(&task_id, "done", Some(&notes_text), None).await?;
            println!("Done: {} — {}", t.id, t.title);
        }

        TasksAction::Blocked { id, task, notes, .. } => {
            let task_id = resolve_task_id(id, task)?;
            let t = ts.update_status(&task_id, "blocked", None, notes.as_deref()).await?;
            println!("Blocked: {} — {}", t.id, t.title);
        }

        TasksAction::Heartbeat { id, task, agent, .. } => {
            let task_id = resolve_task_id(id, task)?;
            ts.heartbeat_task(&task_id, &agent).await?;
            // Silent success — hook calls this fire-and-forget
        }

        TasksAction::Add { title, repo, phase, severity, file } => {
            let repo_path = repo.as_deref().unwrap_or(".");
            let id = format!("{:x}", rand_u64());
            let t = ts.add_task(
                &id, &title, None,
                phase.as_deref(), None, None,
                Some(&severity), file.as_deref(), None, None, None, None,
                repo_path,
            ).await?;
            println!("Added: {} — {}", t.id, t.title);
        }

        TasksAction::Log { id, task, agent, action, detail, notes, entry_type, repo } => {
            let repo_path = repo.as_deref().unwrap_or(".");
            let task_id = id.or(task);
            // Accept --detail or --notes as the detail field
            let detail_text = detail.or(notes);
            ts.log_activity(
                &agent,
                task_id.as_deref(),
                None,
                &action,
                &entry_type,
                detail_text.as_deref(),
                None,
                repo_path,
            ).await?;
            // Silent — called by PostToolUse hook fire-and-forget
        }

        TasksAction::Note { id, task, phase, text, note, agent, repo } => {
            let repo_path = repo.as_deref().unwrap_or(".");
            let task_id = id.or(task);
            let note_text = text.or(note)
                .ok_or_else(|| anyhow::anyhow!("note text required (positional or --note)"))?;
            ts.post_note(&agent, task_id.as_deref(), phase.as_deref(), &note_text, repo_path).await?;
            println!("Note posted.");
        }

        TasksAction::FromPlanning { file, repo } => {
            let repo_path = repo.as_deref().unwrap_or(".");
            let content = tokio::fs::read_to_string(&file).await
                .map_err(|e| anyhow::anyhow!("Cannot read file {}: {e}", file.display()))?;
            let parsed = clawd::tasks::markdown_parser::parse_active_md(&content);
            if parsed.is_empty() {
                println!("No tasks found in {}", file.display());
            } else {
                let count = ts.backfill_from_tasks(parsed, repo_path).await?;
                println!("Imported {count} new task(s) from {}", file.display());
            }
        }

        TasksAction::Sync { repo, active_md } => {
            let repo_path = repo.as_deref().unwrap_or(".");
            let md_path = active_md.unwrap_or_else(|| {
                std::path::PathBuf::from(repo_path).join(".claude/tasks/active.md")
            });
            let content = tokio::fs::read_to_string(&md_path).await
                .map_err(|e| anyhow::anyhow!("Cannot read {}: {e}", md_path.display()))?;
            let parsed = clawd::tasks::markdown_parser::parse_active_md(&content);
            let count = ts.backfill_from_tasks(parsed, repo_path).await?;
            clawd::tasks::queue_serializer::flush_queue(&ts, repo_path).await?;
            println!("Synced: {count} new task(s), queue.json updated.");
        }

        TasksAction::Summary { repo, json } => {
            let summary = ts.summary(repo.as_deref()).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                let done = summary["done"].as_i64().unwrap_or(0);
                let in_progress = summary["in_progress"].as_i64().unwrap_or(0);
                let pending = summary["pending"].as_i64().unwrap_or(0);
                let blocked = summary["blocked"].as_i64().unwrap_or(0);
                let total = summary["total"].as_i64().unwrap_or(0);
                let avg = summary["avg_duration_minutes"].as_f64();
                let bar = "━".repeat(40);
                println!("Task Summary");
                println!("{bar}");
                if let Some(r) = &repo {
                    println!("Project:     {r}");
                }
                println!("Total:       {total}");
                println!("Done:        {done}");
                println!("In Progress: {in_progress}");
                println!("Pending:     {pending}");
                println!("Blocked:     {blocked}");
                if let Some(m) = avg {
                    println!("Avg time:    {m:.1}m per task");
                }
            }
        }

        TasksAction::Activity { repo, task, phase, limit } => {
            let rows = ts.query_activity(&ActivityQueryParams {
                repo_path: repo,
                task_id: task,
                phase,
                limit: Some(limit),
                ..Default::default()
            }).await?;
            if rows.is_empty() {
                println!("No activity found.");
            } else {
                for r in &rows {
                    let task_label = r.task_id.as_deref().unwrap_or("-");
                    println!("[{}] {} | {} | {} | {}",
                        r.ts, r.agent, r.action, task_label,
                        r.detail.as_deref().unwrap_or(""));
                }
            }
        }
    }

    Ok(())
}

fn print_task_detail(t: &AgentTaskRow) {
    println!("ID:       {}", t.id);
    println!("Title:    {}", t.title);
    println!("Status:   {}", t.status);
    println!("Severity: {}", t.severity.as_deref().unwrap_or("-"));
    println!("Phase:    {}", t.phase.as_deref().unwrap_or("-"));
    println!("File:     {}", t.file.as_deref().unwrap_or("-"));
    if let Some(ref a) = t.claimed_by {
        println!("Claimed:  {a}");
    }
    if let Some(ref n) = t.notes {
        println!("Notes:    {n}");
    }
    if let Some(ref b) = t.block_reason {
        println!("Blocked:  {b}");
    }
    println!("Repo:     {}", t.repo_path);
    println!("Created:  {}", t.created_at);
}

fn rand_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
    let pid = std::process::id() as u64;
    // simple non-crypto ID
    (ns as u64).wrapping_mul(1_000_003).wrapping_add(pid)
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
            let mut consecutive_prune_failures: u32 = 0;
            loop {
                match storage.prune_old_sessions(prune_days).await {
                    Ok(n) if n > 0 => {
                        consecutive_prune_failures = 0;
                        info!(pruned = n, days = prune_days, "pruned old sessions");
                    }
                    Ok(_) => {
                        consecutive_prune_failures = 0;
                    }
                    Err(e) => {
                        consecutive_prune_failures += 1;
                        if consecutive_prune_failures >= 3 {
                            warn!(
                                err = %e,
                                failures = consecutive_prune_failures,
                                "session pruning failing repeatedly"
                            );
                        } else {
                            warn!(err = %e, "session pruning failed");
                        }
                    }
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

    // ── Task storage (shared pool from main storage) ──────────────────────────
    let task_storage = Arc::new(TaskStorage::new(storage.pool().clone()));

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
        task_storage: task_storage.clone(),
    });

    // ── Spawn task background jobs ────────────────────────────────────────────
    {
        let ts = task_storage.clone();
        let bc = broadcaster.clone();
        tokio::spawn(clawd::tasks::jobs::run_heartbeat_checker(ts, bc, 90));
    }
    {
        let ts = task_storage.clone();
        tokio::spawn(clawd::tasks::jobs::run_done_task_archiver(ts, 24));
    }
    {
        let ts = task_storage.clone();
        tokio::spawn(clawd::tasks::jobs::run_activity_log_pruner(ts, 30));
    }

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
