// SPDX-License-Identifier: MIT
//! Cost estimator — USD cost lookup by model and token count.
//!
//! Uses a static pricing table (priced per 1M tokens).  Unknown model IDs
//! return 0.0 so future models don't break cost accounting — they just show
//! $0 until the table is updated.
//!
//! Prices are as of 2025-H2. Update `PRICING_TABLE` when Anthropic or OpenAI
//! change their pricing.

/// Pricing entry for one model (per 1 million tokens).
struct ModelPricing {
    model_id: &'static str,
    /// USD per 1M input tokens.
    input_per_mtok: f64,
    /// USD per 1M output tokens.
    output_per_mtok: f64,
}

/// Static pricing table.
///
/// Sources (2025-H2):
/// - claude-haiku-4-5:   $0.25 / $1.25 per MTok (Anthropic)
/// - claude-sonnet-4-6:  $3.00 / $15.00 per MTok (Anthropic)
/// - claude-opus-4-6:    $15.00 / $75.00 per MTok (Anthropic)
/// - gpt-4o:             $2.50 / $10.00 per MTok (OpenAI)
const PRICING_TABLE: &[ModelPricing] = &[
    ModelPricing {
        model_id: "claude-haiku-4-5",
        input_per_mtok: 0.25,
        output_per_mtok: 1.25,
    },
    ModelPricing {
        model_id: "claude-haiku-4-5-20251001",
        input_per_mtok: 0.25,
        output_per_mtok: 1.25,
    },
    ModelPricing {
        model_id: "claude-sonnet-4-6",
        input_per_mtok: 3.00,
        output_per_mtok: 15.00,
    },
    ModelPricing {
        model_id: "claude-sonnet-4-5",
        input_per_mtok: 3.00,
        output_per_mtok: 15.00,
    },
    ModelPricing {
        model_id: "claude-opus-4-6",
        input_per_mtok: 15.00,
        output_per_mtok: 75.00,
    },
    ModelPricing {
        model_id: "gpt-4o",
        input_per_mtok: 2.50,
        output_per_mtok: 10.00,
    },
    ModelPricing {
        model_id: "gpt-4o-mini",
        input_per_mtok: 0.15,
        output_per_mtok: 0.60,
    },
];

/// Estimate the USD cost for a given model and token counts.
///
/// Returns `0.0` for unknown model IDs (not an error — future models may not
/// be in the table yet).  Result is rounded to 6 decimal places.
pub fn estimate_cost(model_id: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    // Prefix-match so "claude-sonnet-4-6-20251015" (versioned) matches "claude-sonnet-4-6".
    let pricing = PRICING_TABLE
        .iter()
        .find(|p| model_id.starts_with(p.model_id) || model_id == p.model_id);

    let Some(p) = pricing else {
        return 0.0;
    };

    let input_cost = (input_tokens as f64 / 1_000_000.0) * p.input_per_mtok;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * p.output_per_mtok;

    // Round to 6 decimal places.
    let total = input_cost + output_cost;
    (total * 1_000_000.0).round() / 1_000_000.0
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haiku_cost() {
        // 1M input + 1M output at haiku pricing
        let cost = estimate_cost("claude-haiku-4-5", 1_000_000, 1_000_000);
        // $0.25 input + $1.25 output = $1.50
        assert!((cost - 1.50).abs() < 0.000001, "expected 1.50, got {cost}");
    }

    #[test]
    fn test_sonnet_cost() {
        // 1M input + 1M output at sonnet pricing
        let cost = estimate_cost("claude-sonnet-4-6", 1_000_000, 1_000_000);
        // $3.00 + $15.00 = $18.00
        assert!((cost - 18.0).abs() < 0.000001, "expected 18.0, got {cost}");
    }

    #[test]
    fn test_opus_cost() {
        let cost = estimate_cost("claude-opus-4-6", 1_000_000, 1_000_000);
        // $15.00 + $75.00 = $90.00
        assert!((cost - 90.0).abs() < 0.000001, "expected 90.0, got {cost}");
    }

    #[test]
    fn test_gpt4o_cost() {
        let cost = estimate_cost("gpt-4o", 1_000_000, 1_000_000);
        // $2.50 + $10.00 = $12.50
        assert!((cost - 12.5).abs() < 0.000001, "expected 12.5, got {cost}");
    }

    #[test]
    fn test_unknown_model_returns_zero() {
        let cost = estimate_cost("some-future-model-5-0", 1_000_000, 1_000_000);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_zero_tokens_cost() {
        let cost = estimate_cost("claude-sonnet-4-6", 0, 0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_small_token_count() {
        // 1000 input + 500 output at haiku pricing
        let cost = estimate_cost("claude-haiku-4-5", 1_000, 500);
        // input: 0.001 * 0.25 = 0.00025; output: 0.0005 * 1.25 = 0.000625; total = 0.000875
        assert!((cost - 0.000875).abs() < 0.000001, "got {cost}");
    }

    #[test]
    fn test_prefix_match_versioned_model() {
        // Versioned model ID like "claude-sonnet-4-6-20251015" should match "claude-sonnet-4-6"
        let cost = estimate_cost("claude-sonnet-4-6-20251015", 1_000_000, 0);
        assert!((cost - 3.0).abs() < 0.000001, "expected 3.0, got {cost}");
    }
}
