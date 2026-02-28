//! Cost-aware routing helpers.
//!
//! Provides per-model cost configs, model recommendation by role/complexity,
//! and a cost estimation function. Values are approximate and may drift from
//! provider pricing — update as needed.

// ── Model cost config ────────────────────────────────────────────────────────

/// Cost per 1,000 tokens (input and output separately) in USD.
#[derive(Debug, Clone)]
pub struct ModelCostConfig {
    pub input_per_1k_tokens: f64,
    pub output_per_1k_tokens: f64,
}

/// Return the cost config for a known model identifier.
///
/// Falls back to a conservative default for unrecognised models.
pub fn get_model_cost(model: &str) -> ModelCostConfig {
    match model {
        "claude-opus-4-6" => ModelCostConfig {
            input_per_1k_tokens: 0.015,
            output_per_1k_tokens: 0.075,
        },
        "claude-sonnet-4-6" => ModelCostConfig {
            input_per_1k_tokens: 0.003,
            output_per_1k_tokens: 0.015,
        },
        "claude-haiku-4-5-20251001" | "claude-haiku" => ModelCostConfig {
            input_per_1k_tokens: 0.00025,
            output_per_1k_tokens: 0.00125,
        },
        _ => ModelCostConfig {
            input_per_1k_tokens: 0.001,
            output_per_1k_tokens: 0.002,
        },
    }
}

// ── Model recommendation ─────────────────────────────────────────────────────

/// Recommend the most cost-effective model for a given agent `role` and task
/// `complexity`.
///
/// Returns a model identifier string. Routing hierarchy:
/// - Lightweight triage/routing work → Haiku (cheapest).
/// - High-complexity planning / architecture → Opus (most capable).
/// - Everything else → Sonnet (balanced).
pub fn recommend_model(role: &str, complexity: &str) -> &'static str {
    match (role, complexity) {
        ("router", _) => "claude-haiku-4-5-20251001",
        ("planner", "high") => "claude-opus-4-6",
        ("planner", _) => "claude-sonnet-4-6",
        ("reviewer", _) => "claude-sonnet-4-6",
        ("implementer", "high") => "claude-sonnet-4-6",
        ("implementer", _) => "claude-haiku-4-5-20251001",
        _ => "claude-sonnet-4-6",
    }
}

// ── Cost estimation ──────────────────────────────────────────────────────────

/// Estimate the USD cost for a single request.
///
/// `input_tokens` — tokens consumed from the prompt / context.
/// `output_tokens` — tokens generated in the response.
/// `model`         — model identifier (see `get_model_cost`).
pub fn estimate_cost(input_tokens: u64, output_tokens: u64, model: &str) -> f64 {
    let config = get_model_cost(model);
    (input_tokens as f64 / 1_000.0) * config.input_per_1k_tokens
        + (output_tokens as f64 / 1_000.0) * config.output_per_1k_tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_estimation_is_nonzero_for_known_models() {
        let cost = estimate_cost(1_000, 500, "claude-sonnet-4-6");
        assert!(cost > 0.0);
    }

    #[test]
    fn router_role_maps_to_haiku() {
        assert_eq!(
            recommend_model("router", "low"),
            "claude-haiku-4-5-20251001"
        );
        assert_eq!(
            recommend_model("router", "high"),
            "claude-haiku-4-5-20251001"
        );
    }

    #[test]
    fn high_complexity_planner_maps_to_opus() {
        assert_eq!(recommend_model("planner", "high"), "claude-opus-4-6");
    }
}
