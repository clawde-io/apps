use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

const DEFAULT_PORT: u16 = 4300;
const DEFAULT_MAX_SESSIONS: usize = 10;
const DEFAULT_MAX_ACCOUNTS: usize = 10;
const DEFAULT_API_BASE_URL: &str = "https://api.clawde.io";
const DEFAULT_RELAY_URL: &str = "wss://api.clawde.io/relay/ws";
const DEFAULT_PRUNE_DAYS: u32 = 30;

// ─── TOML config file ─────────────────────────────────────────────────────────

/// Per-provider configuration profile.
///
/// Parsed from TOML sections like `[provider.claude]`, `[provider.codex]`, etc.
#[derive(Debug, Clone, Deserialize, Default, serde::Serialize)]
pub struct ProviderProfile {
    /// Request timeout in seconds (default: 300).
    pub timeout: Option<u64>,
    /// Maximum tokens for AI responses (default: provider-specific).
    pub max_tokens: Option<u64>,
    /// Prefix prepended to the system prompt for this provider.
    pub system_prompt_prefix: Option<String>,
}

/// `{data_dir}/config.toml` — all fields are optional overrides.
/// Priority: CLI / env var  >  TOML  >  built-in default.
#[derive(Deserialize, Default)]
struct TomlConfig {
    /// WebSocket server port (default: 4300).
    port: Option<u16>,
    /// Maximum concurrent sessions; 0 = unlimited (default: 10).
    max_sessions: Option<usize>,
    /// Maximum registered accounts in the pool; 0 = unlimited (default: 10).
    max_accounts: Option<usize>,
    /// Log level filter string, e.g. "debug", "info,clawd=trace" (default: "info").
    log: Option<String>,
    /// License JWT for verifying relay / auto-switch features. Omit for Free tier.
    license_token: Option<String>,
    /// Override the ClawDE API base URL (default: https://api.clawde.io).
    api_base_url: Option<String>,
    /// Override the relay WebSocket URL (default: wss://api.clawde.io/relay/ws).
    relay_url: Option<String>,
    /// How many days of idle/error sessions to keep before pruning (default: 30; 0 = never).
    session_prune_days: Option<u32>,
    /// Per-provider configuration profiles (e.g. `[provider.claude]`).
    provider: Option<std::collections::HashMap<String, ProviderProfile>>,
}

fn load_toml(data_dir: &Path) -> Option<TomlConfig> {
    let path = data_dir.join("config.toml");
    let contents = std::fs::read_to_string(&path).ok()?;
    match toml::from_str::<TomlConfig>(&contents) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            error!(path = %path.display(), err = %e, "failed to parse config.toml — using defaults");
            None
        }
    }
}

// ─── DaemonConfig ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub port: u16,
    pub data_dir: PathBuf,
    pub log: String,
    pub max_sessions: usize,
    /// Maximum registered accounts in the pool (0 = unlimited, default: 10).
    pub max_accounts: usize,
    /// How many days before idle/error sessions are pruned (0 = never).
    pub session_prune_days: u32,
    /// JWT for license verification (CLAWD_LICENSE_TOKEN env var).
    /// None means Free tier — no verification attempt.
    pub license_token: Option<String>,
    /// Backend API base URL (CLAWD_API_URL env var, default: https://api.clawde.io).
    pub api_base_url: String,
    /// Relay WebSocket URL (CLAWD_RELAY_URL env var).
    pub relay_url: String,
    /// Per-provider profiles (e.g. timeout, max_tokens, system_prompt_prefix).
    pub providers: std::collections::HashMap<String, ProviderProfile>,
}

impl DaemonConfig {
    /// Build config from CLI/env args + optional TOML file.
    ///
    /// Priority (highest to lowest):
    ///   1. CLI / env — passed as `Some(value)` from clap
    ///   2. TOML file at `{data_dir}/config.toml`
    ///   3. Built-in defaults
    pub fn new(
        port: Option<u16>,
        data_dir: Option<PathBuf>,
        log: Option<String>,
        max_sessions: Option<usize>,
    ) -> Self {
        let data_dir = data_dir.unwrap_or_else(default_data_dir);

        // Load TOML as the lowest-priority override layer
        let toml = load_toml(&data_dir).unwrap_or_default();

        let port = port.or(toml.port).unwrap_or(DEFAULT_PORT);
        let log = log.or(toml.log).unwrap_or_else(|| "info".to_string());
        let max_sessions = max_sessions
            .or(toml.max_sessions)
            .unwrap_or(DEFAULT_MAX_SESSIONS);
        let max_accounts = toml.max_accounts.unwrap_or(DEFAULT_MAX_ACCOUNTS);
        let session_prune_days = toml.session_prune_days.unwrap_or(DEFAULT_PRUNE_DAYS);

        let api_base_url = std::env::var("CLAWD_API_URL")
            .ok()
            .or(toml.api_base_url)
            .unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string());

        let relay_url = std::env::var("CLAWD_RELAY_URL")
            .ok()
            .or(toml.relay_url)
            .unwrap_or_else(|| DEFAULT_RELAY_URL.to_string());

        let license_token = std::env::var("CLAWD_LICENSE_TOKEN")
            .ok()
            .filter(|t| !t.is_empty())
            .or(toml.license_token);

        let providers = toml.provider.unwrap_or_default();

        Self {
            port,
            data_dir,
            log,
            max_sessions,
            max_accounts,
            session_prune_days,
            license_token,
            api_base_url,
            relay_url,
            providers,
        }
    }

    /// Get the provider profile for a specific provider name, if configured.
    pub fn provider_profile(&self, name: &str) -> Option<&ProviderProfile> {
        self.providers.get(name)
    }
}

// ─── Hot-reloadable config subset ─────────────────────────────────────────────

/// Non-critical config fields that can be changed without restarting the daemon.
#[derive(Debug, Clone)]
pub struct HotConfig {
    pub log_level: String,
    pub session_prune_days: u32,
}

/// Watches `config.toml` for changes and reloads non-critical fields.
///
/// The watcher uses the `notify` crate (kqueue on macOS, inotify on Linux)
/// to detect file modifications. Only `log_level` and `session_prune_days`
/// are reloaded; port, max_sessions, and other startup-only fields require
/// a full restart.
pub struct ConfigWatcher {
    pub hot: Arc<RwLock<HotConfig>>,
    // Hold the watcher alive; dropping it stops the file watch.
    _watcher: notify_debouncer_full::Debouncer<
        notify_debouncer_full::notify::RecommendedWatcher,
        notify_debouncer_full::FileIdMap,
    >,
}

impl ConfigWatcher {
    /// Start watching `{data_dir}/config.toml` for changes.
    ///
    /// Returns `None` if the watcher could not be created (non-fatal; the
    /// daemon runs fine without hot-reload).
    pub fn start(data_dir: &Path) -> Option<Self> {
        let config_path = data_dir.join("config.toml");
        let initial = load_hot_config(&config_path);
        let hot = Arc::new(RwLock::new(initial));

        let hot_clone = hot.clone();
        let config_path_clone = config_path.clone();
        let rt_handle = tokio::runtime::Handle::current();

        let watcher = notify_debouncer_full::new_debouncer(
            std::time::Duration::from_secs(2),
            None,
            move |result: notify_debouncer_full::DebounceEventResult| {
                if let Ok(events) = result {
                    // Only act on modify/create events
                    let relevant = events.iter().any(|e| {
                        use notify_debouncer_full::notify::EventKind;
                        matches!(
                            e.event.kind,
                            EventKind::Modify(_) | EventKind::Create(_)
                        )
                    });
                    if relevant {
                        let hot = hot_clone.clone();
                        let path = config_path_clone.clone();
                        rt_handle.spawn(async move {
                            let new_config = load_hot_config(&path);
                            let mut guard = hot.write().await;
                            if guard.log_level != new_config.log_level
                                || guard.session_prune_days != new_config.session_prune_days
                            {
                                info!(
                                    log_level = %new_config.log_level,
                                    prune_days = new_config.session_prune_days,
                                    "config.toml reloaded"
                                );
                                *guard = new_config;
                            }
                        });
                    }
                }
            },
        );

        match watcher {
            Ok(mut debouncer) => {
                use notify_debouncer_full::notify::Watcher as _;
                // Watch the data_dir (parent of config.toml) since watching a
                // non-existent file fails on some platforms.
                let watch_path = config_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."));
                if let Err(e) = debouncer.watcher().watch(
                    watch_path,
                    notify_debouncer_full::notify::RecursiveMode::NonRecursive,
                ) {
                    warn!("config watcher failed to start: {e} — hot-reload disabled");
                    return None;
                }
                info!(path = %config_path.display(), "config hot-reload watcher started");
                Some(Self {
                    hot,
                    _watcher: debouncer,
                })
            }
            Err(e) => {
                warn!("config watcher creation failed: {e} — hot-reload disabled");
                None
            }
        }
    }
}

/// Load only the hot-reloadable fields from config.toml.
fn load_hot_config(path: &Path) -> HotConfig {
    let toml = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str::<TomlConfig>(&s).ok())
        .unwrap_or_default();
    HotConfig {
        log_level: toml.log.unwrap_or_else(|| "info".to_string()),
        session_prune_days: toml.session_prune_days.unwrap_or(DEFAULT_PRUNE_DAYS),
    }
}

fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        // ~/Library/Application Support/clawd
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("clawd");
        }
    }
    #[cfg(target_os = "linux")]
    {
        // $XDG_DATA_HOME/clawd or ~/.local/share/clawd
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("clawd");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("clawd");
        }
    }
    #[cfg(target_os = "windows")]
    {
        // %APPDATA%\clawd
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("clawd");
        }
    }
    // Fallback
    PathBuf::from(".clawd")
}
