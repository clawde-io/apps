// SPDX-License-Identifier: MIT
//! Builder Mode — guided project scaffolding for ClawDE.
//!
//! Builder Mode walks a developer through creating a new project from a
//! curated set of stack templates.  The daemon generates a complete directory
//! scaffold with working boilerplate, then hands off to an AI session for
//! continued development.
//!
//! Exposed RPC methods:
//!   `builder.createSession` — start a new builder session for a stack
//!   `builder.listTemplates` — list all available stack templates
//!   `builder.getStatus`     — get current status of a builder session

pub mod handlers;
pub mod model;
pub mod templates;
