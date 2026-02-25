// SPDX-License-Identifier: MIT
//! Performance and scalability module — Sprint Z, SC2.T01–SC2.T05.
//!
//! Provides two focused subsystems:
//!
//! - [`connection_pool`] — managed pool of outbound WebSocket connections with
//!   logical stream multiplexing.  Used by the relay client and multi-daemon
//!   topology features to reduce TLS handshake overhead at scale.
//!
//! - [`wal_tuning`] — SQLite WAL optimisation helpers: apply performance
//!   PRAGMAs at startup, trigger checkpoints on clean shutdown, and run
//!   integrity checks.
//!
//! # Wiring
//!
//! See `sprint_Z_wiring_notes.md` for the exact lines to add to
//! `apps/daemon/src/lib.rs` to expose these modules.

pub mod connection_pool;
pub mod wal_tuning;

pub use connection_pool::{ConnectionPool, PoolConfig, PoolMetrics, StreamId};
pub use wal_tuning::{apply_wal_tuning, checkpoint_wal, integrity_check, WalCheckpointResult};
