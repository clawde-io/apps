// SPDX-License-Identifier: MIT
// BrowserTool — headless browser screenshot integration (Sprint L, VM.T03).
//
// This module provides daemon-side browser automation for AI self-testing.
// The daemon can capture a screenshot of any URL by spawning a headless
// Chromium/Chrome process, then base64-encoding the resulting PNG for
// inclusion in an AI message turn.
//
// Supported browsers (checked in order):
//   chromium, chrome, google-chrome, chromium-browser
//
// Graceful degradation: when no headless browser is found on PATH,
// `BrowserError { code: "no_browser" }` is returned rather than panicking.

pub mod handlers;
pub mod model;
pub mod runner;
