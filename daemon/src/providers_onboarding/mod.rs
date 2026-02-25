// SPDX-License-Identifier: MIT
//! Provider Onboarding & GCI/AID Generation (Sprint I, PO.T01–PO.T19).
//!
//! This module handles:
//! - Detecting installed AI provider CLIs (claude, codex, cursor) and their auth state
//! - Generating personalised Global Claude Instructions (CLAUDE.md) from a questionnaire
//! - Generating Codex AGENTS.md and Cursor rules files
//! - Bootstrapping project-level AI config files (VISION.md + FEATURES.md stubs)
//! - RPC handlers for all provider onboarding RPCs

pub mod scanner;
pub mod gci_generator;
pub mod aid_bootstrapper;
pub mod handlers;
