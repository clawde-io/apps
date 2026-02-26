//! Task Automations — Sprint CC CA.1-CA.9.
//!
//! Automations are trigger-action rules that run automatically when events
//! occur in the daemon (session complete, task done, file saved, cron).

pub mod builtins;
pub mod config;
pub mod engine;
