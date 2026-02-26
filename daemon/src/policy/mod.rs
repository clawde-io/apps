//! `policy` — ClawDE daemon policy engine.
//!
//! This module contains all policy enforcement logic for the agent task
//! execution pipeline:
//!
//! - **Risk classification** — maps tool names to risk levels.
//! - **Approval routing** — manages human-approval request / grant / deny flow.
//! - **MCP trust** — tracks which MCP servers are trusted and what tools they
//!   may invoke.
//! - **Supply-chain verification** — detects unexpected changes to MCP server
//!   binaries.
//! - **Sandbox** — enforces path-escape and network-access boundaries.
//! - **Output scanning** — redacts secrets from tool results before display.
//! - **Secrets guard** — prevents raw credentials from being passed as tool
//!   arguments.
//! - **Policy hooks** — `pre_tool` and `post_tool` entry points for the MCP
//!   dispatcher.
//! - **DoD checker** — validates Definition-of-Done gates before a task
//!   transitions to CodeReview.
//! - **Scanners** — thin wrappers around placeholder and secrets scanners.
//! - **RBAC** — role-based access control for agent tool dispatch.

pub mod approval;
pub mod dod;
pub mod engine;
pub mod hooks;
pub mod mcp_trust;
pub mod output_scan;
pub mod rbac;
pub mod risk;
pub mod rules;
pub mod sandbox;
pub mod scanners;
pub mod secrets;
pub mod supply_chain;
pub mod tester;

// ─── Top-level re-exports ─────────────────────────────────────────────────────

pub use engine::{PolicyDecision, PolicyEngine};
pub use sandbox::PolicyViolation;
