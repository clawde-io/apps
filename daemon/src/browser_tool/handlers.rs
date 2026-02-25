// SPDX-License-Identifier: MIT
//! RPC handlers for the Browser Tool — Sprint L (VM.T03)
//!
//! Exposed methods:
//! - `browser.screenshot` — capture a screenshot of a URL

use crate::browser_tool::model::BrowserConfig;
use crate::browser_tool::runner::BrowserToolRunner;
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `browser.screenshot` — take a screenshot of a URL.
///
/// Params:
/// - `url`: string — the URL to capture
/// - `width`: optional integer — viewport width (default: 1280)
/// - `height`: optional integer — viewport height (default: 900)
/// - `timeout_ms`: optional integer — timeout in ms (default: 15000)
pub async fn screenshot(params: Value, _ctx: &AppContext) -> Result<Value> {
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("url required"))?;

    let config = BrowserConfig {
        headless: true,
        viewport_width: params.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32,
        viewport_height: params.get("height").and_then(|v| v.as_u64()).unwrap_or(900) as u32,
        timeout_secs: params
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(15_000)
            / 1000,
    };

    match BrowserToolRunner::take_screenshot(url, &config).await {
        Ok(result) => Ok(serde_json::to_value(result)?),
        Err(e) => Ok(json!({
            "error": {
                "code": e.code,
                "message": e.message,
            }
        })),
    }
}
