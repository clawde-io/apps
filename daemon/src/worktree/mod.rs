//! Task-scoped Git worktree management.
//!
//! Each code-modifying task gets its own isolated Git worktree (`claw/<id>-<slug>`
//! branch), preventing concurrent tasks from touching each other's in-progress
//! changes.
//!
//! This is DISTINCT from `session::worktree`, which handles per-session
//! (detached-HEAD) worktrees. This module handles per-task (branched) worktrees
//! with write-path enforcement and merge discipline.

pub mod cleanup;
pub mod health;
pub mod manager;
pub mod merge;

pub use manager::{SharedWorktreeManager, WorktreeInfo, WorktreeManager, WorktreeStatus};
