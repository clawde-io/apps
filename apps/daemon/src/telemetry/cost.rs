//! Cost estimation for AI provider usage.
//!
//! Rates are sourced from publicly listed pricing pages and kept as constants
//! here.  They must be updated when provider pricing changes.
//!
//! All costs are expressed as USD per 1 000 tokens.

// ─── Model rate table ─────────────────────────────────────────────────────────

/// Returns `(input_per_1k_usd, output_per_1k_usd)` for a model identifier.
///
/// The model string may be a full API model name (e.g. `"claude-opus-4-6"`) or
/// a short alias.  Unrecognised models fall back to a conservative mid-range rate.
pub fn get_model_rates(model: &str) -> (f64, f64) {
    let m = model.to_lowercase();

    // ── Anthropic / Claude ────────────────────────────────────────────────────
    if m.contains("claude-opus-4") || m.contains("opus-4") {
        return (0.015, 0.075);
    }
    if m.contains("claude-sonnet-4") || m.contains("sonnet-4") {
        return (0.003, 0.015);
    }
    if m.contains("claude-haiku-3-5") || m.contains("haiku-3-5") {
        return (0.0008, 0.004);
    }
    if m.contains("claude-haiku") {
        return (0.00025, 0.00125);
    }
    if m.contains("claude-sonnet") {
        return (0.003, 0.015);
    }
    if m.contains("claude-opus") {
        return (0.015, 0.075);
    }
    if m.contains("claude") {
        // Generic Claude fallback
        return (0.003, 0.015);
    }

    // ── OpenAI / Codex ────────────────────────────────────────────────────────
    if m.contains("gpt-4o") {
        return (0.005, 0.015);
    }
    if m.contains("gpt-4-turbo") {
        return (0.010, 0.030);
    }
    if m.contains("gpt-4") {
        return (0.030, 0.060);
    }
    if m.contains("gpt-3.5") {
        return (0.0005, 0.0015);
    }
    if m.contains("codex") || m.contains("code-davinci") {
        return (0.002, 0.002);
    }

    // ── Unknown model — use a conservative mid-range estimate ─────────────────
    (0.005, 0.015)
}

// ─── Cost calculation ─────────────────────────────────────────────────────────

/// Estimate total cost in USD for a single provider call.
///
/// # Arguments
/// * `input_tokens`  — number of prompt/input tokens consumed
/// * `output_tokens` — number of completion/output tokens produced
/// * `model`         — model identifier string (used to look up rates)
pub fn estimate_cost_usd(input_tokens: u64, output_tokens: u64, model: &str) -> f64 {
    let (input_rate, output_rate) = get_model_rates(model);
    let input_cost = (input_tokens as f64 / 1_000.0) * input_rate;
    let output_cost = (output_tokens as f64 / 1_000.0) * output_rate;
    input_cost + output_cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_tokens_zero_cost() {
        assert_eq!(estimate_cost_usd(0, 0, "claude-sonnet-4-6"), 0.0);
    }

    #[test]
    fn known_model_rates() {
        let (i, o) = get_model_rates("claude-opus-4-6");
        assert!(i > 0.0);
        assert!(o > i); // output always more expensive than input for Opus
    }

    #[test]
    fn unknown_model_fallback() {
        let (i, o) = get_model_rates("some-future-model-9");
        assert!(i > 0.0 && o > 0.0);
    }
}
