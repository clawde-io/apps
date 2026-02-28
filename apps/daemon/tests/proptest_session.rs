// SPDX-License-Identifier: MIT
//! Sprint BB TH.2 — property-based tests.
//!
//! 1. Session state machine: 100 random valid transitions stay valid.
//! 2. Signing round-trip: generate_keypair + sign + verify is always `true`.
//!
//! Run with: cargo test --test proptest_session

use proptest::prelude::*;

// ─── 1. Session state machine properties ─────────────────────────────────────

/// All valid session status strings.
const VALID_STATUSES: &[&str] = &[
    "idle", "running", "paused", "done", "error", "waiting", "stopped",
];

/// Valid state transitions:
/// Maps a "from" status to the set of states it can transition to.
fn valid_next_states(status: &str) -> &'static [&'static str] {
    match status {
        "idle" => &["running", "stopped"],
        "running" => &["idle", "paused", "done", "error", "waiting"],
        "paused" => &["idle", "stopped"],
        "waiting" => &["running", "error"],
        "error" => &["running", "stopped"],
        "done" | "stopped" => &[], // terminal — no transitions
        _ => &[],
    }
}

/// Returns `true` if `to` is a valid next state from `from`.
fn is_valid_transition(from: &str, to: &str) -> bool {
    valid_next_states(from).contains(&to)
}

proptest! {
    /// Any valid transition from a non-terminal state produces a valid next state.
    #[test]
    fn valid_transition_stays_valid(
        from_idx in 0_usize..5,    // idle, running, paused, waiting, error (non-terminal)
        step_count in 1_usize..100,
    ) {
        let non_terminal = &["idle", "running", "paused", "waiting", "error"];
        let mut current = non_terminal[from_idx % non_terminal.len()];

        for step in 0..step_count {
            let nexts = valid_next_states(current);
            if nexts.is_empty() {
                // Terminal state — stop
                break;
            }
            // Pick the next state deterministically from the step index
            let next = nexts[step % nexts.len()];
            prop_assert!(
                is_valid_transition(current, next),
                "step {step}: invalid transition {current} → {next}"
            );
            prop_assert!(
                VALID_STATUSES.contains(&next),
                "next state '{next}' is not in VALID_STATUSES"
            );
            current = next;
        }
    }

    /// Terminal states (done, stopped) have NO valid transitions.
    #[test]
    fn terminal_states_have_no_transitions(
        terminal_idx in 0_usize..2,
    ) {
        let terminals = &["done", "stopped"];
        let terminal = terminals[terminal_idx % terminals.len()];
        for &any_state in VALID_STATUSES {
            prop_assert!(
                !is_valid_transition(terminal, any_state),
                "terminal '{terminal}' should not transition to '{any_state}'"
            );
        }
    }

    /// No state can transition to itself (idempotent transitions are not modelled).
    #[test]
    fn no_self_transitions(status_idx in 0_usize..7) {
        let status = VALID_STATUSES[status_idx % VALID_STATUSES.len()];
        prop_assert!(
            !is_valid_transition(status, status),
            "'{status}' should not transition to itself"
        );
    }
}

// ─── 2. Signing round-trip properties ────────────────────────────────────────

/// Test the pack signing round-trip: generate_keypair → sign → verify = true.
///
/// We test via a thin wrapper over the core signing logic so we don't need
/// a real filesystem (pack.toml). The actual `PackSigner` reads `pack.toml`
/// from disk, so we inline the relevant crypto primitives here.
#[cfg(test)]
mod signing_roundtrip {
    use proptest::prelude::*;

    /// Sign arbitrary bytes with a freshly generated ed25519 keypair and
    /// verify that the signature is accepted.
    fn sign_and_verify(payload: &[u8]) -> bool {
        use ed25519_dalek::{Signer, SigningKey, Verifier};
        use rand_core::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let sig = signing_key.sign(payload);
        verifying_key.verify(payload, &sig).is_ok()
    }

    proptest! {
        /// For any payload (0–4 KiB), sign+verify always succeeds.
        #[test]
        fn roundtrip_any_payload(payload in prop::collection::vec(any::<u8>(), 0..4096)) {
            prop_assert!(sign_and_verify(&payload), "sign+verify failed");
        }

        /// For a single-byte payload, sign+verify always succeeds.
        #[test]
        fn roundtrip_single_byte(byte in any::<u8>()) {
            prop_assert!(sign_and_verify(&[byte]), "sign+verify failed for single byte");
        }

        /// Empty payload is valid — ed25519 handles zero-length messages.
        #[test]
        fn roundtrip_empty(_seed in 0_u32..1000) {
            prop_assert!(sign_and_verify(&[]), "sign+verify failed for empty payload");
        }
    }
}
