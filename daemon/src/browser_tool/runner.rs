// SPDX-License-Identifier: MIT
// BrowserToolRunner — headless browser screenshot capture (Sprint L, VM.T03).
//
// Strategy:
//   1. detect_browser() searches PATH for a supported browser binary.
//   2. take_screenshot() spawns the browser with --headless, --screenshot,
//      --window-size=WxH, and a --no-sandbox flag for common Linux/CI setups.
//   3. The browser writes `screenshot.png` to the working directory.
//   4. The file is read, validated for size, and base64-encoded.
//
// Graceful degradation: if detect_browser() returns None, take_screenshot()
// returns a BrowserError immediately without spawning any process.

use crate::browser_tool::model::{BrowserConfig, BrowserError, ScreenshotResult};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Maximum encoded image size (10 MB raw PNG bytes).
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

/// Browser binaries to probe, in preference order.
const CANDIDATE_BROWSERS: &[&str] = &["chromium", "chrome", "google-chrome", "chromium-browser"];

/// The daemon-side runner for headless browser screenshots.
pub struct BrowserToolRunner;

impl BrowserToolRunner {
    /// Detect the first headless-capable browser binary on PATH.
    ///
    /// Returns the binary name (e.g. `"chromium"`) or `None` if none
    /// of the candidates can be found via `which`.
    pub fn detect_browser() -> Option<String> {
        for candidate in CANDIDATE_BROWSERS {
            if which_browser(candidate) {
                debug!(browser = *candidate, "headless browser detected on PATH");
                return Some((*candidate).to_string());
            }
        }
        None
    }

    /// Take a screenshot of `url` using the first available headless browser.
    ///
    /// Returns `Ok(ScreenshotResult)` on success, or `Err(BrowserError)` when
    /// no browser is available or the browser process fails.
    ///
    /// # Errors
    ///
    /// Never panics. Errors are represented as typed `BrowserError` values so
    /// the JSON-RPC handler can serialise them without leaking internal state.
    pub async fn take_screenshot(
        url: &str,
        config: &BrowserConfig,
    ) -> Result<ScreenshotResult, BrowserError> {
        // 1. Find a browser.
        let browser = match Self::detect_browser() {
            Some(b) => b,
            None => return Err(BrowserError::no_browser()),
        };

        // 2. Create a temp directory for output isolation.
        //    The browser writes `screenshot.png` into the CWD.
        let tmp = TempDir::new().map_err(|e| BrowserError::spawn_failed(&e.to_string()))?;
        let tmp_path = tmp.path();
        let screenshot_path = tmp_path.join("screenshot.png");

        // 3. Build the command.
        let window_size = format!("{},{}", config.viewport_width, config.viewport_height);
        let mut cmd = Command::new(&browser);
        cmd.arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg("--screenshot")
            .arg(screenshot_path.to_string_lossy().as_ref())
            .arg(format!("--window-size={window_size}"))
            .arg(url)
            .current_dir(tmp_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        debug!(browser = %browser, url = %url, "spawning headless browser");

        // 4. Spawn and wait with a timeout.
        let mut child = cmd
            .spawn()
            .map_err(|e| BrowserError::spawn_failed(&e.to_string()))?;

        let wait_result = timeout(Duration::from_secs(config.timeout_secs), child.wait()).await;

        match wait_result {
            Err(_elapsed) => {
                // Timeout — kill the child to avoid zombie processes.
                let _ = child.kill().await;
                warn!(url = %url, secs = config.timeout_secs, "browser screenshot timed out");
                return Err(BrowserError::timeout(config.timeout_secs));
            }
            Ok(Err(e)) => {
                return Err(BrowserError::spawn_failed(&e.to_string()));
            }
            Ok(Ok(status)) => {
                if !status.success() {
                    // Non-zero exit — browser may print useful info to stderr,
                    // but we don't surface it to callers (could contain paths).
                    warn!(url = %url, status = ?status, "browser exited with non-zero status");
                    // Fall through: check if a partial screenshot was written.
                }
            }
        }

        // 5. Read the output file.
        if !screenshot_path.exists() {
            return Err(BrowserError::no_output());
        }

        read_and_encode_png(&screenshot_path, url, config, &browser)
    }
}

/// Check if a browser binary is available on PATH using `which` semantics.
///
/// We use std::process (synchronous) because this is called from a sync context
/// in tests and the overhead is negligible (single stat per candidate).
fn which_browser(binary: &str) -> bool {
    // Use the PATH variable to locate the binary.
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let candidate = Path::new(dir).join(binary);
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

/// Read a PNG file from disk, validate its size, and base64-encode it.
fn read_and_encode_png(
    path: &Path,
    url: &str,
    config: &BrowserConfig,
    browser: &str,
) -> Result<ScreenshotResult, BrowserError> {
    let bytes = std::fs::read(path).map_err(|e| BrowserError::read_failed(&e.to_string()))?;

    if bytes.is_empty() {
        return Err(BrowserError::no_output());
    }

    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(BrowserError::size_exceeded(bytes.len()));
    }

    let png_base64 = BASE64.encode(&bytes);

    Ok(ScreenshotResult {
        url: url.to_string(),
        png_base64,
        width: config.viewport_width,
        height: config.viewport_height,
        captured_at: Utc::now().to_rfc3339(),
        browser_used: browser.to_string(),
    })
}
