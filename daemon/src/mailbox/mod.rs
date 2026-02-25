// SPDX-License-Identifier: MIT
// Sprint N: Multi-Repo Orchestration — mailbox subsystem.
//
// Exposes:
//   - model    — MailboxMessage, MailboxPolicy
//   - storage  — MailboxStorage (SQLite-backed)
//   - watcher  — MailboxWatcher (filesystem watcher for .claude/inbox/)
//   - handlers — JSON-RPC 2.0 handler functions

pub mod handlers;
pub mod model;
pub mod storage;
pub mod watcher;
