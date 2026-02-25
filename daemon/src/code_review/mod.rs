// SPDX-License-Identifier: MIT
//! AI Code Review Engine — Sprint O (CR.T01–CR.T18)
//!
//! Provides:
//! - Tool integration layer: spawn external linters, parse output, aggregate findings
//! - Codegraph builder: parse diffs to detect changed functions and breaking changes
//! - AI synthesis: group findings by theme, synthesize into coherent review comments
//! - Review workflow: orchestrate the full pipeline and compute a grade
//! - RPC handlers: `review.run`, `review.fix`, `review.learn`

pub mod model;
pub mod tool_runner;
pub mod codegraph;
pub mod ai_synthesis;
pub mod workflow;
pub mod handlers;

pub use model::{
    Grade, ReviewComment, ReviewConfig, ReviewIssue, ReviewResult, ReviewSeverity, ToolConfig,
    ToolResult,
};
