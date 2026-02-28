// metrics/cost.rs — Per-provider cost calculator (Sprint PP OB.2).
//
// Cost per 1M tokens (USD) as of 2026-02. Update when provider pricing changes.
// Input tokens are billed at the listed rate; output tokens at 3–5× input rate
// for most models.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostModel {
    // Anthropic
    ClaudeOpus46,
    ClaudeSonnet46,
    ClaudeHaiku45,
    // OpenAI
    Gpt53Codex,
    CodexSpark,
    // Generic (when model is unknown)
    Unknown,
}

impl CostModel {
    /// Derive cost model from provider name + model string.
    pub fn from_provider_model(provider: &str, model: &str) -> Self {
        match provider {
            "claude" => match model {
                m if m.contains("opus") => Self::ClaudeOpus46,
                m if m.contains("haiku") => Self::ClaudeHaiku45,
                _ => Self::ClaudeSonnet46,
            },
            "codex" => match model {
                "codex-spark" => Self::CodexSpark,
                _ => Self::Gpt53Codex,
            },
            _ => Self::Unknown,
        }
    }

    /// Cost per 1M input tokens in USD.
    pub fn input_cost_per_m(&self) -> f64 {
        match self {
            Self::ClaudeOpus46 => 15.00,
            Self::ClaudeSonnet46 => 3.00,
            Self::ClaudeHaiku45 => 0.80,
            Self::Gpt53Codex => 10.00,
            Self::CodexSpark => 1.50,
            Self::Unknown => 3.00, // conservative estimate
        }
    }

    /// Cost per 1M output tokens in USD.
    pub fn output_cost_per_m(&self) -> f64 {
        match self {
            Self::ClaudeOpus46 => 75.00,
            Self::ClaudeSonnet46 => 15.00,
            Self::ClaudeHaiku45 => 4.00,
            Self::Gpt53Codex => 30.00,
            Self::CodexSpark => 6.00,
            Self::Unknown => 15.00,
        }
    }

    /// Calculate total cost in USD for a given token count.
    pub fn calculate_cost(&self, tokens_in: i64, tokens_out: i64) -> f64 {
        let input_cost = (tokens_in as f64 / 1_000_000.0) * self.input_cost_per_m();
        let output_cost = (tokens_out as f64 / 1_000_000.0) * self.output_cost_per_m();
        input_cost + output_cost
    }
}

/// Calculate cost for a message exchange.
pub fn calculate_cost(provider: &str, model: &str, tokens_in: i64, tokens_out: i64) -> f64 {
    CostModel::from_provider_model(provider, model).calculate_cost(tokens_in, tokens_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sonnet_cost() {
        let cost = calculate_cost("claude", "claude-sonnet-4-6", 1_000_000, 1_000_000);
        // $3 input + $15 output = $18
        assert!((cost - 18.0).abs() < 0.01, "Sonnet cost = {}", cost);
    }

    #[test]
    fn test_haiku_cheap() {
        let haiku = CostModel::ClaudeHaiku45;
        let sonnet = CostModel::ClaudeSonnet46;
        assert!(haiku.input_cost_per_m() < sonnet.input_cost_per_m());
    }

    #[test]
    fn test_zero_tokens() {
        let cost = calculate_cost("claude", "claude-opus-4-6", 0, 0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_unknown_provider() {
        let cost = calculate_cost("unknown", "unknown-model", 100_000, 50_000);
        assert!(cost > 0.0);
    }
}
