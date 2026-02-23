//! Provider and model routing for agent roles (Phase 43e).

use crate::agents::capabilities::{select_provider, Provider, SelectionContext};
use crate::agents::roles::AgentRole;

/// The result of routing an agent: which provider and model to use.
pub struct RoutingDecision {
    pub role: AgentRole,
    pub provider: Provider,
    pub model: String,
    pub reason: String,
}

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
    let model = default_model_for(&provider, role);
    RoutingDecision {
        role: role.clone(),
        reason: format!("role={}, complexity={}", role.as_str(), complexity),
        model,
        provider,
    }
}

/// Return the default model string for a provider+role combination.
pub fn default_model_for(provider: &Provider, role: &AgentRole) -> String {
    match (provider, role) {
        (Provider::Claude, AgentRole::Router) => "claude-haiku-4-5-20251001".to_string(),
        (Provider::Claude, _) => "claude-sonnet-4-6".to_string(),
        (Provider::Codex, _) => "codex-mini-latest".to_string(),
        _ => "claude-sonnet-4-6".to_string(),
    }
}
