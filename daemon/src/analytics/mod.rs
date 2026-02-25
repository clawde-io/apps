// SPDX-License-Identifier: MIT
//! Personal analytics subsystem — Sprint Q (AN.T01–AN.T05).
//!
//! Provides personal usage statistics, per-provider breakdowns, per-session
//! analytics, and the achievement system. All data is read from the existing
//! `storage` tables — no new migrations are needed for the core analytics query
//! layer; achievements add one new table (`analytics_achievements`).

pub mod achievements;
pub mod handlers;
pub mod model;
pub mod storage;
