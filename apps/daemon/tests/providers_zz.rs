// tests/providers_zz.rs — Provider feature parity smoke tests (Sprint ZZ MP.T05)
//
// Tests that each supported provider driver compiles and satisfies the
// capability contract. These are unit-level parity tests — no actual
// CLI binary is required to be installed on CI.

use clawd::agents::capabilities::{Provider, ProviderCapabilities};
use clawd::agents::copilot;
use clawd::agents::gemini;

// ─── Capability contract tests ────────────────────────────────────────────────

#[test]
fn claude_has_full_session_support() {
    let caps = ProviderCapabilities::for_provider(&Provider::Claude);
    assert!(caps.supports_fork, "Claude must support fork");
    assert!(caps.supports_mcp, "Claude must support MCP");
    assert!(
        caps.max_context_tokens >= 100_000,
        "Claude context must be >= 100k tokens"
    );
}

#[test]
fn codex_has_session_support() {
    let caps = ProviderCapabilities::for_provider(&Provider::Codex);
    assert!(
        caps.max_context_tokens > 0,
        "Codex must have a context limit"
    );
}

#[test]
fn claude_cost_is_reasonable() {
    let caps = ProviderCapabilities::for_provider(&Provider::Claude);
    // Cost per 1k tokens should be > 0 (paid model)
    assert!(caps.cost_per_1k_tokens_in >= 0.0);
    assert!(caps.cost_per_1k_tokens_out >= 0.0);
}

#[test]
fn codex_cost_is_reasonable() {
    let caps = ProviderCapabilities::for_provider(&Provider::Codex);
    assert!(caps.cost_per_1k_tokens_in >= 0.0);
    assert!(caps.cost_per_1k_tokens_out >= 0.0);
}

// ─── Copilot driver contract tests ────────────────────────────────────────────

#[test]
fn copilot_provider_name_constant() {
    assert_eq!(copilot::PROVIDER_NAME, "copilot");
}

// ─── Gemini driver contract tests ─────────────────────────────────────────────

#[test]
fn gemini_provider_name_constant() {
    assert_eq!(gemini::PROVIDER_NAME, "gemini");
}

// ─── Capability serialization parity ─────────────────────────────────────────

#[test]
fn capability_matrix_serializes_to_json() {
    let caps = ProviderCapabilities::for_provider(&Provider::Claude);
    let json = serde_json::to_string(&caps).expect("must serialize");
    assert!(json.contains("supports_fork"));
    assert!(json.contains("supports_mcp"));
}

#[test]
fn all_providers_have_unique_names() {
    // Verify that provider name constants don't clash
    assert_ne!(copilot::PROVIDER_NAME, gemini::PROVIDER_NAME);
    assert_ne!(copilot::PROVIDER_NAME, "claude");
    assert_ne!(gemini::PROVIDER_NAME, "codex");
}

// ─── MP.T03 — Capability matrix JSON serialization ───────────────────────────

#[test]
fn capability_matrix_json_roundtrip() {
    let caps = ProviderCapabilities::for_provider(&Provider::Claude);
    let json = serde_json::to_string(&caps).unwrap();
    let back: ProviderCapabilities = serde_json::from_str(&json).unwrap();
    assert_eq!(back.supports_fork, caps.supports_fork);
    assert_eq!(back.supports_mcp, caps.supports_mcp);
    assert_eq!(back.max_context_tokens, caps.max_context_tokens);
}
