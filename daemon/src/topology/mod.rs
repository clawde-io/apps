// SPDX-License-Identifier: MIT
// Sprint N: Multi-Repo Orchestration — topology subsystem.
//
// Exposes:
//   - model     — RepoNode, Dependency, TopologyGraph, DepType
//   - storage   — TopologyStorage (SQLite-backed)
//   - detector  — auto_detect_dependencies (heuristic scanner)
//   - handlers  — JSON-RPC 2.0 handler functions

pub mod detector;
pub mod handlers;
pub mod model;
pub mod storage;
