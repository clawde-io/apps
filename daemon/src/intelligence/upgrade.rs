/// Auto-upgrade on failure — evaluates response quality and upgrades model if needed.
///
/// This is the post-response hook in the pre-send pipeline (doc 61).
/// Maximum one auto-upgrade per message to prevent runaway cost.

use super::model_router::ModelSelection;
use super::RunnerOutput;
use crate::config::ModelIntelligenceConfig;

// ─── Quality evaluation ───────────────────────────────────────────────────────

/// Refusal signal phrases that indicate the model declined or couldn't complete the task.
const REFUSAL_PHRASES: &[&str] = &[
    "i cannot",
    "i'm unable to",
    "i am unable to",
    "as an ai",
    "i don't have the ability",
    "i can't do that",
    "i'm not able to",
];

/// Reason the response was considered poor quality.
#[derive(Debug, Clone, PartialEq)]
pub enum PoorReason {
    /// The provider returned a tool call error (schema invalid, tool not found, etc.).
    ToolCallError,
    /// The output appears truncated (empty or suspiciously short for the task).
    OutputTruncated,
    /// The model explicitly refused the task.
    ModelRefusal,
    /// No content returned at all.
    EmptyResponse,
}

/// Quality assessment of a completed provider turn.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseQuality {
    /// Response looks good — no upgrade needed.
    Ok,
    /// Response is poor quality for the given reason.
    Poor(PoorReason),
}

/// Evaluate the quality of a runner output.
///
/// This function is **pure** — no side effects, no async, no panics.
pub fn evaluate_response(output: &RunnerOutput) -> ResponseQuality {
    if output.content.is_empty() {
        return ResponseQuality::Poor(PoorReason::EmptyResponse);
    }
    if output.tool_call_error {
        return ResponseQuality::Poor(PoorReason::ToolCallError);
    }
    if output.output_truncated {
        return ResponseQuality::Poor(PoorReason::OutputTruncated);
    }
    let lower = output.content.to_lowercase();
    for phrase in REFUSAL_PHRASES {
        if lower.contains(phrase) {
            return ResponseQuality::Poor(PoorReason::ModelRefusal);
        }
    }
    ResponseQuality::Ok
}

// ─── Upgrade logic ────────────────────────────────────────────────────────────

/// Attempt to upgrade to the next model tier.
///
/// Returns `None` if already at the maximum configured model or upgrade count exceeded.
/// The `upgrade_count` parameter tracks how many upgrades have been attempted this turn;
/// callers must enforce max 1 upgrade per message.
pub fn upgrade_model(
    current: &ModelSelection,
    config: &ModelIntelligenceConfig,
    upgrade_count: u8,
) -> Option<ModelSelection> {
    // Max 1 auto-upgrade per message.
    if upgrade_count >= 1 {
        return None;
    }

    let current_lower = current.model_id.to_lowercase();

    // Upgrade chain: haiku → sonnet → opus
    let (next_model, reason) = if current_lower.contains("haiku") {
        (
            config.provider_models.sonnet.clone(),
            "auto_upgrade:haiku→sonnet".to_string(),
        )
    } else if current_lower.contains("sonnet") {
        // Only upgrade to opus if config allows it
        if config.max_model == "sonnet" || config.max_model == "haiku" {
            return None; // cap prevents upgrade
        }
        (
            config.provider_models.opus.clone(),
            "auto_upgrade:sonnet→opus".to_string(),
        )
    } else {
        // Already at opus or unknown — cannot upgrade further
        return None;
    };

    // Check the next model doesn't exceed max_model cap
    let next_tier = model_tier(&next_model);
    let max_tier = model_tier(&config.max_model);
    if next_tier > max_tier {
        return None;
    }

    Some(ModelSelection {
        provider: provider_for_model(&next_model),
        model_id: next_model,
        reason,
    })
}

fn model_tier(model: &str) -> u8 {
    let lower = model.to_lowercase();
    if lower.contains("opus") || lower == "opus" {
        3
    } else if lower.contains("sonnet") || lower == "sonnet" {
        2
    } else {
        1
    }
}

fn provider_for_model(model: &str) -> String {
    if model.to_lowercase().starts_with("claude") {
        "claude".to_string()
    } else {
        "claude".to_string()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModelIntelligenceConfig;

    fn runner_ok(model: &str) -> RunnerOutput {
        RunnerOutput {
            content: "Here is the implementation you requested.".to_string(),
            tool_call_error: false,
            output_truncated: false,
            model_id: model.to_string(),
            input_tokens: 100,
            output_tokens: 200,
        }
    }

    fn runner_empty() -> RunnerOutput {
        RunnerOutput {
            content: String::new(),
            tool_call_error: false,
            output_truncated: false,
            model_id: "claude-haiku-4-5".to_string(),
            input_tokens: 0,
            output_tokens: 0,
        }
    }

    fn runner_refusal() -> RunnerOutput {
        RunnerOutput {
            content: "I'm unable to complete this task as an AI.".to_string(),
            tool_call_error: false,
            output_truncated: false,
            model_id: "claude-haiku-4-5".to_string(),
            input_tokens: 50,
            output_tokens: 20,
        }
    }

    fn haiku_selection() -> ModelSelection {
        ModelSelection {
            model_id: "claude-haiku-4-5".to_string(),
            provider: "claude".to_string(),
            reason: "auto_select:Simple".to_string(),
        }
    }

    fn sonnet_selection() -> ModelSelection {
        ModelSelection {
            model_id: "claude-sonnet-4-6".to_string(),
            provider: "claude".to_string(),
            reason: "auto_select:Moderate".to_string(),
        }
    }

    #[test]
    fn ok_response_no_upgrade() {
        let q = evaluate_response(&runner_ok("claude-haiku-4-5"));
        assert_eq!(q, ResponseQuality::Ok);
    }

    #[test]
    fn empty_response_is_poor() {
        let q = evaluate_response(&runner_empty());
        assert_eq!(q, ResponseQuality::Poor(PoorReason::EmptyResponse));
    }

    #[test]
    fn refusal_is_poor() {
        let q = evaluate_response(&runner_refusal());
        assert_eq!(q, ResponseQuality::Poor(PoorReason::ModelRefusal));
    }

    #[test]
    fn tool_call_error_is_poor() {
        let mut out = runner_ok("claude-haiku-4-5");
        out.tool_call_error = true;
        let q = evaluate_response(&out);
        assert_eq!(q, ResponseQuality::Poor(PoorReason::ToolCallError));
    }

    #[test]
    fn haiku_upgrades_to_sonnet() {
        let cfg = ModelIntelligenceConfig::default();
        let sel = upgrade_model(&haiku_selection(), &cfg, 0);
        assert!(sel.is_some());
        let sel = sel.unwrap();
        assert!(sel.model_id.contains("sonnet"), "got: {}", sel.model_id);
    }

    #[test]
    fn max_one_upgrade_per_message() {
        let cfg = ModelIntelligenceConfig::default();
        let sel = upgrade_model(&haiku_selection(), &cfg, 1);
        assert!(sel.is_none(), "upgrade_count=1 should prevent upgrade");
    }

    #[test]
    fn sonnet_upgrades_to_opus_when_allowed() {
        let cfg = ModelIntelligenceConfig {
            max_model: "opus".to_string(),
            ..Default::default()
        };
        let sel = upgrade_model(&sonnet_selection(), &cfg, 0);
        assert!(sel.is_some());
        assert!(sel.unwrap().model_id.contains("opus"));
    }

    #[test]
    fn sonnet_cannot_upgrade_when_capped_at_sonnet() {
        let cfg = ModelIntelligenceConfig {
            max_model: "sonnet".to_string(),
            ..Default::default()
        };
        let sel = upgrade_model(&sonnet_selection(), &cfg, 0);
        assert!(sel.is_none(), "sonnet capped at sonnet should block upgrade");
    }
}
