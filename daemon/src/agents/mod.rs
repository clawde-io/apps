//! Provider SDK integration modules (Phase 43j).
//!
//! This module provides the bridge layer between the clawd daemon and the
//! external AI provider SDKs (Claude Code Agent SDK and Codex app-server).
//! It covers:
//!   - Provider capability definitions and selection logic
//!   - MCP server connections for both Claude and Codex
//!   - Config file generation for both providers
//!   - The Claude Code Agent SDK session API
//!   - The Codex app-server thread API
//!
//! Orchestration modules (Phase 43e) are wired in below.

pub mod capabilities;
pub mod claude_config;
pub mod claude_mcp;
pub mod claude_sdk;
pub mod codex_appserver;
pub mod codex_config;
pub mod codex_mcp;

// ─── Sprint ZZ: Additional provider agents ────────────────────────────────
pub mod copilot;
pub mod gemini;

// ─── Orchestration modules (Phase 43e) ───────────────────────────────────────
pub mod health;
pub mod implementer;
pub mod lifecycle;
pub mod orchestrator;
pub mod planner;
pub mod prompts;
pub mod prompt_cache;
pub mod provider_session;
pub mod qa;
pub mod reviewer;
pub mod roles;
pub mod router;
pub mod routing;
