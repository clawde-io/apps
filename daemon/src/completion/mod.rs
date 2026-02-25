// SPDX-License-Identifier: MIT
// Code Completion Engine — fill-in-middle completions via the active session's provider
// (Sprint K, CC.T01–CC.T03).
//
// The completion module sends a structured fill-in-middle prompt to the AI
// provider and extracts the suggested code text.  It is session-agnostic:
// the caller supplies a session ID and the daemon routes to the correct runner.

pub mod handlers;
pub mod model;
