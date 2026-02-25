// SPDX-License-Identifier: MIT
// BrowserTool data model types (Sprint L, VM.T03).

use serde::{Deserialize, Serialize};

/// Configuration for a headless browser screenshot session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    /// Run browser in headless mode (no visible window).
    /// Always true for daemon use; exposed for testing.
    #[serde(default = "default_headless")]
    pub headless: bool,

    /// Screenshot timeout in seconds. Defaults to 15.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Viewport width in pixels. Defaults to 1280.
    #[serde(default = "default_viewport_width")]
    pub viewport_width: u32,

    /// Viewport height in pixels. Defaults to 720.
    #[serde(default = "default_viewport_height")]
    pub viewport_height: u32,
}

fn default_headless() -> bool {
    true
}

fn default_timeout_secs() -> u64 {
    15
}

fn default_viewport_width() -> u32 {
    1280
}

fn default_viewport_height() -> u32 {
    720
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            headless: true,
            timeout_secs: 15,
            viewport_width: 1280,
            viewport_height: 720,
        }
    }
}

/// A successfully captured screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotResult {
    /// The URL that was captured.
    pub url: String,

    /// Base64-encoded PNG image data (standard alphabet, no line breaks).
    /// Empty string if no output file was produced (should not occur on success).
    pub png_base64: String,

    /// Actual viewport width used for the capture.
    pub width: u32,

    /// Actual viewport height used for the capture.
    pub height: u32,

    /// RFC 3339 timestamp of when the screenshot was taken.
    pub captured_at: String,

    /// Name of the browser binary that was used (e.g. "chromium").
    pub browser_used: String,
}

/// An error returned when the browser tool cannot complete the screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserError {
    /// Machine-readable error code.
    ///
    /// Known values:
    ///   `no_browser`       — no headless browser found on PATH
    ///   `spawn_failed`     — browser process could not be started
    ///   `timeout`          — browser did not complete within timeout
    ///   `no_output`        — browser exited successfully but produced no PNG
    ///   `read_failed`      — output PNG could not be read from disk
    ///   `size_exceeded`    — PNG exceeds the 10 MB base64 limit
    pub code: String,

    /// Human-readable description of the error.
    pub message: String,
}

impl BrowserError {
    pub fn no_browser() -> Self {
        Self {
            code: "no_browser".to_string(),
            message:
                "No headless browser found on PATH. Install Chromium or Chrome and ensure \
                 one of these binaries is available: chromium, chrome, google-chrome, \
                 chromium-browser."
                    .to_string(),
        }
    }

    pub fn spawn_failed(detail: &str) -> Self {
        Self {
            code: "spawn_failed".to_string(),
            message: format!("Failed to start browser process: {detail}"),
        }
    }

    pub fn timeout(secs: u64) -> Self {
        Self {
            code: "timeout".to_string(),
            message: format!("Browser did not produce output within {secs} seconds"),
        }
    }

    pub fn no_output() -> Self {
        Self {
            code: "no_output".to_string(),
            message: "Browser exited but produced no screenshot file".to_string(),
        }
    }

    pub fn read_failed(detail: &str) -> Self {
        Self {
            code: "read_failed".to_string(),
            message: format!("Could not read browser output: {detail}"),
        }
    }

    pub fn size_exceeded(bytes: usize) -> Self {
        Self {
            code: "size_exceeded".to_string(),
            message: format!(
                "Screenshot is too large ({bytes} bytes). The 10 MB base64 limit was exceeded."
            ),
        }
    }
}
