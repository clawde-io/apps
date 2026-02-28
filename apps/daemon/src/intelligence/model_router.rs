/// Model router — maps TaskComplexity to the optimal model, respecting user config and pins.
///
/// This is Stage 0b in the pre-send pipeline. It runs after the classifier and before
/// the context optimizer. It never fails — if the preferred model is unavailable, the
/// fallback chain (opus → sonnet → haiku) guarantees a valid selection.
use super::classifier::TaskComplexity;
use crate::config::ModelIntelligenceConfig;
use tracing::info;

// ─── Types ────────────────────────────────────────────────────────────────────

/// The model + provider selected for a given AI turn.
#[derive(Debug, Clone)]
pub struct ModelSelection {
    /// Model identifier sent to the provider (e.g. "claude-sonnet-4-6").
    pub model_id: String,
    /// Provider name (e.g. "claude", "codex").
    pub provider: String,
    /// Human-readable reason for this selection (for debug logs and audit trail).
    pub reason: String,
}

// ─── Router ───────────────────────────────────────────────────────────────────

/// Select the optimal model for a task.
///
/// Priority order:
/// 1. `model_pin` — user's explicit per-session override (always wins)
/// 2. `auto_select = false` — use the complexity floor model for everything
/// 3. Auto-select based on task complexity, respecting `max_model` cap
///
/// The function never returns an error — it always falls back to the safest option.
pub fn select_model(
    complexity: TaskComplexity,
    model_pin: Option<&str>,
    config: &ModelIntelligenceConfig,
) -> ModelSelection {
    // ── 1. Session pin overrides everything ───────────────────────────────────
    if let Some(pin) = model_pin {
        let sel = ModelSelection {
            model_id: pin.to_string(),
            provider: provider_for_model(pin),
            reason: "session_pin".to_string(),
        };
        info!(model = %sel.model_id, reason = %sel.reason, "model selected");
        return sel;
    }

    // ── 2. Auto-select disabled — use complexity floor model ─────────────────
    if !config.auto_select {
        let floor_complexity = parse_complexity_floor(&config.complexity_floor);
        let floor_model = complexity_to_model(&floor_complexity, config);
        let sel = ModelSelection {
            model_id: floor_model.clone(),
            provider: provider_for_model(&floor_model),
            reason: "auto_select_disabled_floor".to_string(),
        };
        info!(model = %sel.model_id, reason = %sel.reason, "model selected");
        return sel;
    }

    // ── 3. Auto-select: complexity → model, capped by max_model ──────────────
    let preferred = complexity_to_model(&complexity, config);
    let capped = apply_max_model_cap(&preferred, config);

    let reason = if capped != preferred {
        format!("auto_select:{:?}_capped_to_{}", complexity, capped)
    } else {
        format!("auto_select:{:?}", complexity)
    };

    let sel = ModelSelection {
        model_id: capped.clone(),
        provider: provider_for_model(&capped),
        reason,
    };
    info!(model = %sel.model_id, reason = %sel.reason, "model selected");
    sel
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Parse the `complexity_floor` string from config into a `TaskComplexity`.
fn parse_complexity_floor(floor: &str) -> TaskComplexity {
    match floor {
        "Moderate" => TaskComplexity::Moderate,
        "Complex" => TaskComplexity::Complex,
        "DeepReasoning" => TaskComplexity::DeepReasoning,
        _ => TaskComplexity::Simple,
    }
}

/// Map TaskComplexity to the configured model for that level.
fn complexity_to_model(complexity: &TaskComplexity, config: &ModelIntelligenceConfig) -> String {
    match complexity {
        TaskComplexity::Simple => config.provider_models.haiku.clone(),
        TaskComplexity::Moderate => config.provider_models.sonnet.clone(),
        TaskComplexity::Complex => config.provider_models.sonnet.clone(),
        TaskComplexity::DeepReasoning => config.provider_models.opus.clone(),
    }
}

/// Apply `max_model` cap: if the selected model is more powerful than allowed, downgrade.
///
/// Model tier ordering: haiku < sonnet < opus.
fn apply_max_model_cap(model: &str, config: &ModelIntelligenceConfig) -> String {
    let max = &config.max_model;
    let current_tier = model_tier(model);
    let cap_tier = model_tier(max.as_str());

    if current_tier > cap_tier {
        // Downgrade to the max allowed model
        match max.as_str() {
            "haiku" => config.provider_models.haiku.clone(),
            "sonnet" => config.provider_models.sonnet.clone(),
            _ => config.provider_models.sonnet.clone(),
        }
    } else {
        model.to_string()
    }
}

/// Returns a numeric tier for a model string (higher = more powerful).
fn model_tier(model: &str) -> u8 {
    let lower = model.to_lowercase();
    if lower.contains("opus") {
        3
    } else if lower.contains("sonnet") {
        2
    } else if lower.contains("haiku") {
        1
    } else {
        // Unknown model — treat as moderate tier
        2
    }
}

/// Determine the provider name from a model ID.
fn provider_for_model(model: &str) -> String {
    let lower = model.to_lowercase();
    if lower.starts_with("claude") {
        "claude".to_string()
    } else if lower.starts_with("gpt") || lower.starts_with("o1") || lower.starts_with("o3") {
        "codex".to_string()
    } else {
        "claude".to_string() // default
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModelIntelligenceConfig;

    fn default_config() -> ModelIntelligenceConfig {
        ModelIntelligenceConfig::default()
    }

    fn config_with_max(max: &str) -> ModelIntelligenceConfig {
        ModelIntelligenceConfig {
            max_model: max.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn simple_routes_to_haiku() {
        let sel = select_model(TaskComplexity::Simple, None, &default_config());
        assert!(sel.model_id.contains("haiku"), "got: {}", sel.model_id);
        assert_eq!(sel.provider, "claude");
    }

    #[test]
    fn moderate_routes_to_sonnet() {
        let sel = select_model(TaskComplexity::Moderate, None, &default_config());
        assert!(sel.model_id.contains("sonnet"), "got: {}", sel.model_id);
    }

    #[test]
    fn deep_reasoning_routes_to_opus() {
        let sel = select_model(TaskComplexity::DeepReasoning, None, &default_config());
        assert!(sel.model_id.contains("opus"), "got: {}", sel.model_id);
    }

    #[test]
    fn pin_overrides_complexity() {
        let sel = select_model(
            TaskComplexity::DeepReasoning,
            Some("claude-haiku-4-5"),
            &default_config(),
        );
        assert_eq!(sel.model_id, "claude-haiku-4-5");
        assert_eq!(sel.reason, "session_pin");
    }

    #[test]
    fn max_model_haiku_caps_opus() {
        let cfg = config_with_max("haiku");
        let sel = select_model(TaskComplexity::DeepReasoning, None, &cfg);
        assert!(sel.model_id.contains("haiku"), "got: {}", sel.model_id);
        assert!(sel.reason.contains("capped"));
    }

    #[test]
    fn auto_select_false_uses_floor() {
        let cfg = ModelIntelligenceConfig {
            auto_select: false,
            complexity_floor: "Moderate".to_string(),
            ..Default::default()
        };
        let sel = select_model(TaskComplexity::Simple, None, &cfg);
        assert!(sel.model_id.contains("sonnet"), "got: {}", sel.model_id);
        assert_eq!(sel.reason, "auto_select_disabled_floor");
    }

    // ── Additional coverage (MI.T25) ─────────────────────────────────────────

    #[test]
    fn complex_routes_to_sonnet() {
        let sel = select_model(TaskComplexity::Complex, None, &default_config());
        assert!(sel.model_id.contains("sonnet"), "got: {}", sel.model_id);
        assert!(sel.reason.contains("Complex"), "got reason: {}", sel.reason);
    }

    #[test]
    fn max_model_sonnet_caps_deep_reasoning() {
        let cfg = config_with_max("sonnet");
        let sel = select_model(TaskComplexity::DeepReasoning, None, &cfg);
        assert!(sel.model_id.contains("sonnet"), "got: {}", sel.model_id);
        assert!(sel.reason.contains("capped"), "got reason: {}", sel.reason);
    }

    #[test]
    fn max_model_haiku_caps_moderate_too() {
        let cfg = config_with_max("haiku");
        let sel = select_model(TaskComplexity::Moderate, None, &cfg);
        assert!(sel.model_id.contains("haiku"), "got: {}", sel.model_id);
    }

    #[test]
    fn pin_overrides_simple_to_opus() {
        let sel = select_model(
            TaskComplexity::Simple,
            Some("claude-opus-4-6"),
            &default_config(),
        );
        assert_eq!(sel.model_id, "claude-opus-4-6");
        assert_eq!(sel.reason, "session_pin");
    }

    #[test]
    fn auto_select_false_deep_floor_always_uses_opus() {
        let cfg = ModelIntelligenceConfig {
            auto_select: false,
            complexity_floor: "DeepReasoning".to_string(),
            ..Default::default()
        };
        let sel = select_model(TaskComplexity::Simple, None, &cfg);
        assert!(sel.model_id.contains("opus"), "got: {}", sel.model_id);
    }

    #[test]
    fn provider_is_claude_for_claude_models() {
        let sel = select_model(TaskComplexity::Simple, None, &default_config());
        assert_eq!(sel.provider, "claude");
    }

    #[test]
    fn provider_is_codex_for_gpt_pin() {
        let sel = select_model(TaskComplexity::Moderate, Some("gpt-4o"), &default_config());
        assert_eq!(sel.provider, "codex");
    }
}
