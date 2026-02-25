// SPDX-License-Identifier: MIT
//! Session Intelligence subsystem (Sprint G).
//!
//! Sub-modules:
//! - `health`      — SI.T06: health score tracking per session
//! - `refresh`     — SI.T07: proactive session refresh when health < 40
//! - `bridge`      — SI.T08: cross-conversation context bridging
//! - `complexity`  — SI.T09-T10: task complexity classification + split proposal
//! - `continuation`— SI.T11-T12: auto-continuation + premature stop detection
//! - `context_guard` — SI.T02-T03: context window guard + compression

pub mod bridge;
pub mod complexity;
pub mod context_guard;
pub mod continuation;
pub mod health;

pub use complexity::TaskComplexity;
pub use health::SessionHealthState;
