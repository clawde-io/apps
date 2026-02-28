//! Evals — rule governance and automated testing.
//!
//! This module provides:
//!  - `versioning`  — SHA-256 hashing of policy files
//!  - `detector`    — change detection and background watching
//!  - `runner`      — fixture-based eval runner
//!  - `judge`       — LLM-as-judge scoring stub
//!  - `budget`      — rule size budget checks
//!  - `rollback`    — policy snapshot/restore
//!  - `report`      — markdown regression report generation
//!  - `scanners`    — policy violation detectors (placeholders, secrets, forbidden tools)

pub mod budget;
pub mod detector;
pub mod judge;
pub mod report;
pub mod rollback;
pub mod runner;
pub mod scanners;
pub mod session_eval;
pub mod versioning;
