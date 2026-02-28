//! Conversation threading module (Phase 43f).
//!
//! This module provides the full lifecycle of conversation threads:
//!
//! - **Control threads** — one per project, persistent, orchestration-only.
//!   Never calls file-write tools. Creates tasks, shows status, requests approvals.
//!
//! - **Task threads** — scoped to one task, run in a git worktree. Context is
//!   isolated from the control thread. Seeded with task spec + repo state only.
//!
//! - **Sub threads** — forked from a task thread for parallel exploration.
//!
//! Thread history is stored in SQLite (`threads` + `thread_turns` tables).
//! Vendor session IDs are cached in memory by [`sessions::SessionSnapshotStore`].

pub mod cleanup;
pub mod context;
pub mod control;
pub mod events;
pub mod model;
pub mod sessions;
pub mod task;
