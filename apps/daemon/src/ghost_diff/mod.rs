//! Ghost Diff — Sprint CC GD.1-GD.8.
//!
//! Compares session file changes against spec files (`.claw/specs/`) to detect
//! spec drift before it compounds into technical debt.

pub mod engine;
pub mod spec_parser;
