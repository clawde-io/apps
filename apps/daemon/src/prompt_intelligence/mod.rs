// SPDX-License-Identifier: MIT
//! Intelligent prompt suggestions subsystem (Sprint W, IP.T01–T08).
//!
//! Provides heuristic-based prompt completion suggestions drawn from:
//! - Prompt history (most-used phrases)
//! - Keyword-triggered templates ("fix …", "add …", "explain …")
//! - Active session context and repo profile

pub mod handlers;
pub mod model;
pub mod suggester;
