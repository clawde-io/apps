// SPDX-License-Identifier: MIT
//! Completion confidence scorer — AE.T12–T13 (Autonomous Execution Engine, Sprint J).
//!
//! Computes a 0.0–1.0 score for how confident the daemon is that a task is
//! truly complete.  The score is stored in `task_reviews.grade` (via the
//! handler layer) and surfaced in the Flutter task card.
//!
//! ## Scoring heuristic
//!
//! The score starts at 0 and gains points from evidence of completeness:
//!
//! | Signal                                      | Weight |
//! |---------------------------------------------|--------|
//! | All DoD items resolved (none unresolved)    | +0.30  |
//! | Requirements fully addressed                | +0.25  |
//! | No stub markers in conversation             | +0.20  |
//! | Session message count ≥ 3 (not trivial)     | +0.15  |
//! | Plan was approved by user                   | +0.10  |
//!
//! The final value is clamped to [0.0, 1.0].

use crate::autonomous::AePlan;
use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

/// Per-task confidence snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskConfidence {
    pub task_id: String,
    /// Confidence in [0.0, 1.0].
    pub score: f32,
    /// Human-readable signals that contributed (or did not) to the score.
    pub signals: Vec<ConfidenceSignal>,
}

/// A single evidence signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceSignal {
    pub name: String,
    pub present: bool,
    pub weight: f32,
}

// ─── ConfidenceScorer ─────────────────────────────────────────────────────────

/// Heuristic confidence scorer.  Runs synchronously — no AI call.
pub struct ConfidenceScorer;

impl ConfidenceScorer {
    /// Compute the completion confidence for `plan` given the conversation so far.
    ///
    /// `session_messages` is the list of assistant message texts (not user turns).
    /// Empty message list yields a low score (no evidence of work done).
    pub fn compute_confidence(plan: &AePlan, session_messages: &[impl AsRef<str>]) -> f32 {
        let mut score = 0.0_f32;
        let messages_text: Vec<&str> = session_messages.iter().map(|m| m.as_ref()).collect();
        let combined = messages_text.join("\n");

        // ── Signal 1: All DoD items resolved ──────────────────────────────────
        // Heuristic: each DoD item should appear (or a near-synonym) somewhere in
        // the assistant's messages.
        let dod_score = if plan.definition_of_done.is_empty() {
            // No DoD defined — skip; give partial credit.
            0.15_f32
        } else {
            let resolved = plan
                .definition_of_done
                .iter()
                .filter(|item| {
                    let key = item.to_ascii_lowercase();
                    combined.to_ascii_lowercase().contains(key.as_str())
                })
                .count();
            let ratio = resolved as f32 / plan.definition_of_done.len() as f32;
            0.30 * ratio
        };
        score += dod_score;

        // ── Signal 2: Requirements addressed ──────────────────────────────────
        let req_score = if plan.requirements.is_empty() {
            0.12_f32 // no explicit requirements — give small partial credit
        } else {
            let addressed = plan
                .requirements
                .iter()
                .filter(|req| {
                    let key = req.to_ascii_lowercase();
                    combined.to_ascii_lowercase().contains(key.as_str())
                })
                .count();
            let ratio = addressed as f32 / plan.requirements.len() as f32;
            0.25 * ratio
        };
        score += req_score;

        // ── Signal 3: No stub markers ─────────────────────────────────────────
        let stub_keywords = ["TODO", "FIXME", "placeholder", "stub", "unimplemented!"];
        let has_stubs = stub_keywords
            .iter()
            .any(|kw| combined.contains(kw));
        if !has_stubs {
            score += 0.20;
        }

        // ── Signal 4: Non-trivial conversation depth ───────────────────────────
        if session_messages.len() >= 3 {
            score += 0.15;
        } else if session_messages.len() == 2 {
            score += 0.07;
        }

        // ── Signal 5: Plan approved by user ───────────────────────────────────
        if plan.approved_at.is_some() {
            score += 0.10;
        }

        score.clamp(0.0, 1.0)
    }

    /// Build the full `TaskConfidence` breakdown for a task.
    pub fn build_task_confidence(
        plan: &AePlan,
        session_messages: &[impl AsRef<str>],
    ) -> TaskConfidence {
        let messages_text: Vec<&str> = session_messages.iter().map(|m| m.as_ref()).collect();
        let combined = messages_text.join("\n");

        // ── Evaluate individual signals ────────────────────────────────────────

        let dod_resolved = if plan.definition_of_done.is_empty() {
            false
        } else {
            plan.definition_of_done.iter().all(|item| {
                combined
                    .to_ascii_lowercase()
                    .contains(item.to_ascii_lowercase().as_str())
            })
        };

        let reqs_addressed = !plan.requirements.is_empty()
            && plan.requirements.iter().all(|req| {
                combined
                    .to_ascii_lowercase()
                    .contains(req.to_ascii_lowercase().as_str())
            });

        let stub_keywords = ["TODO", "FIXME", "placeholder", "stub", "unimplemented!"];
        let no_stubs = !stub_keywords.iter().any(|kw| combined.contains(kw));

        let deep_enough = session_messages.len() >= 3;
        let approved = plan.approved_at.is_some();

        let signals = vec![
            ConfidenceSignal {
                name: "dod_resolved".to_owned(),
                present: dod_resolved,
                weight: 0.30,
            },
            ConfidenceSignal {
                name: "requirements_addressed".to_owned(),
                present: reqs_addressed,
                weight: 0.25,
            },
            ConfidenceSignal {
                name: "no_stub_markers".to_owned(),
                present: no_stubs,
                weight: 0.20,
            },
            ConfidenceSignal {
                name: "conversation_depth".to_owned(),
                present: deep_enough,
                weight: 0.15,
            },
            ConfidenceSignal {
                name: "plan_approved".to_owned(),
                present: approved,
                weight: 0.10,
            },
        ];

        let score = Self::compute_confidence(plan, session_messages);

        TaskConfidence {
            task_id: plan.id.clone(),
            score,
            signals,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomous::PlanGenerator;

    fn make_plan(message: &str) -> AePlan {
        PlanGenerator::generate_plan(message, "sess-test").expect("plan")
    }

    #[test]
    fn test_confidence_empty_messages_is_low() {
        let plan = make_plan("Add login button.");
        let score = ConfidenceScorer::compute_confidence(&plan, &[] as &[&str]);
        assert!(score < 0.5, "Expected low score for empty messages, got {score}");
    }

    #[test]
    fn test_confidence_fully_resolved_plan() {
        let message = "Add OAuth middleware.\n\
            - Must validate JWT tokens\n\
            \nAcceptance criteria:\n- Returns 200 on valid token\n- Returns 401 on invalid token";
        let mut plan = make_plan(message);
        // Mark as approved.
        plan.approved_at = Some(chrono::Utc::now().to_rfc3339());

        let messages: Vec<&str> = vec![
            "I have implemented the JWT validation middleware.",
            "Returns 200 on valid token. Returns 401 on invalid token.",
            "The middleware has been wired into the router. Tests pass.",
        ];

        let score = ConfidenceScorer::compute_confidence(&plan, &messages);
        // Should be near 1.0 given all signals satisfied
        assert!(
            score > 0.5,
            "Expected high confidence for fully resolved plan, got {score}"
        );
    }

    #[test]
    fn test_confidence_stub_markers_reduce_score() {
        let plan = make_plan("Implement feature X.");
        let messages: Vec<&str> = vec![
            "TODO: finish this implementation",
            "FIXME: this is a placeholder",
            "stub: not done yet",
        ];
        let score = ConfidenceScorer::compute_confidence(&plan, &messages);
        // Stub markers should prevent the +0.20 bonus
        let without_stubs = ConfidenceScorer::compute_confidence(&plan, &["done"] as &[&str]);
        assert!(
            score <= without_stubs,
            "Stubs should not increase score: {score} vs {without_stubs}"
        );
    }

    #[test]
    fn test_confidence_score_clamped() {
        let message = "Implement feature X.\n\
            - Must do A\n\
            \nAcceptance criteria:\n- A is done";
        let mut plan = make_plan(message);
        plan.approved_at = Some(chrono::Utc::now().to_rfc3339());
        let messages: Vec<&str> = vec!["A is done", "All clear", "Done"];
        let score = ConfidenceScorer::compute_confidence(&plan, &messages);
        assert!(score <= 1.0, "Score must not exceed 1.0");
        assert!(score >= 0.0, "Score must not be negative");
    }

    #[test]
    fn test_build_task_confidence_returns_signals() {
        let plan = make_plan("Do something.");
        let confidence =
            ConfidenceScorer::build_task_confidence(&plan, &["done"] as &[&str]);
        assert_eq!(confidence.signals.len(), 5);
        assert_eq!(confidence.task_id, plan.id);
    }
}
