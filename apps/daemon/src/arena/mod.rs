// SPDX-License-Identifier: MIT
// Arena Mode — blind provider comparison (Sprint K, AM.T01–AM.T06).
//
// The arena module enables side-by-side comparison of two AI providers
// on the same prompt without revealing which is which until the user votes.
// After 20+ votes the leaderboard drives auto-routing decisions.

pub mod handlers;
pub mod model;
pub mod storage;
