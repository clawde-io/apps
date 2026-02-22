use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::error;

const DEFAULT_PORT: u16 = 4300;
const DEFAULT_MAX_SESSIONS: usize = 10;
const DEFAULT_API_BASE_URL: &str = "https://api.clawde.io";
const DEFAULT_RELAY_URL: &str = "wss://api.clawde.io/relay/ws";
const DEFAULT_PRUNE_DAYS: u32 = 30;

// ─── TOML config file ─────────────────────────────────────────────────────────

/// `{data_dir}/config.toml` — all fields are optional overrides.
/// Priority: CLI / env var  >  TOML  >  built-in default.
#[derive(Deserialize, Default)]
struct TomlConfig {
    /// WebSocket server port (default: 4300).
    port: Option<u16>,
    /// Maximum concurrent sessions; 0 = unlimited (default: 10).
    max_sessions: Option<usize>,
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
    /// How many days before idle/error sessions are pruned (0 = never).
    pub session_prune_days: u32,
    /// JWT for license verification (CLAWD_LICENSE_TOKEN env var).
    /// None means Free tier — no verification attempt.
    pub license_token: Option<String>,
    /// Backend API base URL (CLAWD_API_URL env var, default: https://api.clawde.io).
    pub api_base_url: String,
    /// Relay WebSocket URL (CLAWD_RELAY_URL env var).
    pub relay_url: String,
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

        Self {
            port,
            data_dir,
            log,
            max_sessions,
            session_prune_days,
            license_token,
            api_base_url,
            relay_url,
        }
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
