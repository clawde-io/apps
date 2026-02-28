//! Provider and model routing for agent roles (Phase 43e, Sprint BB PV.2/PV.4).

use crate::agents::capabilities::{select_provider, Provider, ProviderSpeed, SelectionContext};
use crate::agents::roles::AgentRole;

// ─── Model name constants ─────────────────────────────────────────────────────

const CODEX_SPARK: &str = "codex-spark";
const GPT_53_CODEX: &str = "gpt-5.3-codex";

// ─── RoutingDecision ──────────────────────────────────────────────────────────

/// The result of routing an agent: which provider, model, and speed to use.
pub struct RoutingDecision {
    pub role: AgentRole,
    pub provider: Provider,
    pub model: String,
    pub speed: ProviderSpeed,
    pub reason: String,
}

// ─── Speed tier from role ─────────────────────────────────────────────────────

/// Determine the Codex speed tier appropriate for a given agent role.
///
/// Fast roles need low latency (classification, cross-model review, QA tooling).
/// Full roles need highest quality (long-horizon planning, code implementation).
pub fn speed_for_role(role: &AgentRole) -> ProviderSpeed {
    match role {
        AgentRole::Router | AgentRole::Reviewer | AgentRole::QaExecutor => ProviderSpeed::Fast,
        AgentRole::Planner | AgentRole::Implementer => ProviderSpeed::Full,
    }
}

// ─── Main routing entry point ─────────────────────────────────────────────────

/// Select the best provider and model for a given role and task context.
pub fn route_agent(
    role: &AgentRole,
    complexity: &str,
    previous_provider: Option<&Provider>,
    available_providers: &[Provider],
) -> RoutingDecision {
    let ctx = SelectionContext {
        role: role.as_str().to_string(),
        complexity: complexity.to_string(),
        cost_budget_usd: None,
        available_providers: available_providers.to_vec(),
        previous_provider: previous_provider.cloned(),
    };
    let provider = select_provider(&ctx);
    let speed = speed_for_role(role);
    let model = default_model_for(&provider, role, &speed);
    RoutingDecision {
        role: role.clone(),
        reason: format!(
            "role={}, complexity={}, speed={:?}",
            role.as_str(),
            complexity,
            speed
        ),
        model,
        speed,
        provider,
    }
}

// ─── Model selection ──────────────────────────────────────────────────────────

/// Return the default model string for a provider + role + speed combination.
///
/// Claude always uses the same model family regardless of speed; the Router
/// role gets the cheaper Haiku variant.  Codex selects by speed tier.
pub fn default_model_for(provider: &Provider, role: &AgentRole, speed: &ProviderSpeed) -> String {
    match (provider, role, speed) {
        (Provider::Claude, AgentRole::Router, _) => "claude-haiku-4-5-20251001".to_string(),
        (Provider::Claude, _, _) => "claude-sonnet-4-6".to_string(),
        (Provider::Codex, _, ProviderSpeed::Fast) => CODEX_SPARK.to_string(),
        (Provider::Codex, _, ProviderSpeed::Full) => GPT_53_CODEX.to_string(),
        _ => "claude-sonnet-4-6".to_string(),
    }
}
