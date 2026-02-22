use std::path::PathBuf;

const DEFAULT_API_BASE_URL: &str = "https://api.clawde.io";
const DEFAULT_RELAY_URL: &str = "wss://api.clawde.io/relay/ws";

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub port: u16,
    pub data_dir: PathBuf,
    pub log: String,
    pub max_sessions: usize,
    /// JWT for license verification (CLAWD_LICENSE_TOKEN env var).
    /// None means Free tier — no verification attempt.
    pub license_token: Option<String>,
    /// Backend API base URL (CLAWD_API_URL env var, default: https://api.clawde.io).
    pub api_base_url: String,
    /// Relay WebSocket URL (CLAWD_RELAY_URL env var).
    pub relay_url: String,
}

impl DaemonConfig {
    pub fn new(port: u16, data_dir: Option<PathBuf>, log: String) -> Self {
        let data_dir = data_dir.unwrap_or_else(default_data_dir);
        let api_base_url = std::env::var("CLAWD_API_URL")
            .unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_string());
        let relay_url = std::env::var("CLAWD_RELAY_URL")
            .unwrap_or_else(|_| DEFAULT_RELAY_URL.to_string());
        let license_token = std::env::var("CLAWD_LICENSE_TOKEN").ok().filter(|t| !t.is_empty());

        Self {
            port,
            data_dir,
            log,
            max_sessions: 10,
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
