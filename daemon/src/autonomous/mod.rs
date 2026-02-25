// SPDX-License-Identifier: MIT
//! Autonomous Execution Engine (Sprint J, AE.T01–AE.T20).
//!
//! Sub-modules:
//! - `plan_generator`  — AE.T01/T03: orchestration turn + JIRA-style AePlan generation
//! - `drift_enforcer`  — AE.T04–T07: periodic re-injection + drift detection + correction
//! - `qa_executor`     — AE.T08–T11: self-QA gate + cross-provider CR + validator runner
//! - `confidence`      — AE.T12–T13: completion confidence scoring
//! - `recipe`          — AE.T19–T20: WorkflowRecipe schema + recipe engine

pub mod confidence;
pub mod drift_enforcer;
pub mod handlers;
pub mod plan_generator;
pub mod qa_executor;
pub mod recipe;

pub use confidence::{ConfidenceScorer, TaskConfidence};
pub use drift_enforcer::{DriftEnforcer, DriftResult};
pub use plan_generator::{AePlan, PlanGenerator};
pub use qa_executor::{QaExecutor, QaResult, ValidatorOutput};
pub use recipe::{RecipeEngine, WorkflowRecipe};
