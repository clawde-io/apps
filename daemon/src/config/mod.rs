use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub port: u16,
    pub data_dir: PathBuf,
    pub log: String,
    pub max_sessions: usize,
}

impl DaemonConfig {
    pub fn new(port: u16, data_dir: Option<PathBuf>, log: String) -> Self {
        let data_dir = data_dir.unwrap_or_else(default_data_dir);
        Self {
            port,
            data_dir,
            log,
            max_sessions: 10,
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
