// SPDX-License-Identifier: MIT
// Sprint II CH.5 — `clawd chat` CLI tests.
//
// Non-interactive mode and option parsing are unit-tested here.
// Integration tests that require a live daemon are marked #[ignore].

use clawd::cli::chat::ChatOpts;

// ─── Option construction ───────────────────────────────────────────────────────

#[test]
fn chat_opts_defaults() {
    let opts = ChatOpts::default();
    assert!(opts.resume.is_none());
    assert!(!opts.session_list);
    assert!(opts.non_interactive.is_none());
    assert!(opts.provider.is_none());
}

#[test]
fn chat_opts_non_interactive() {
    let opts = ChatOpts {
        non_interactive: Some("What does this code do?".to_string()),
        provider: Some("claude".to_string()),
        ..Default::default()
    };
    assert_eq!(
        opts.non_interactive.as_deref(),
        Some("What does this code do?")
    );
    assert!(!opts.session_list);
}

#[test]
fn chat_opts_resume() {
    let opts = ChatOpts {
        resume: Some("sess-abc123".to_string()),
        ..Default::default()
    };
    assert_eq!(opts.resume.as_deref(), Some("sess-abc123"));
    assert!(opts.non_interactive.is_none());
}

#[test]
fn chat_opts_session_list() {
    let opts = ChatOpts {
        session_list: true,
        ..Default::default()
    };
    assert!(opts.session_list);
    assert!(opts.resume.is_none());
}

// ─── Integration tests (require live daemon) ─────────────────────────────────

/// Verify non-interactive mode connects, sends, and receives a response.
/// Requires a running daemon on port 4300 — skipped in CI.
#[tokio::test]
#[ignore = "requires live daemon on localhost:4300"]
async fn non_interactive_sends_and_receives() {
    use clawd::config::DaemonConfig;

    let config = DaemonConfig::new(None, None, Some("error".to_string()), None, None);
    let opts = ChatOpts {
        non_interactive: Some("Say 'hello' and nothing else.".to_string()),
        provider: Some("claude".to_string()),
        ..Default::default()
    };
    // run_chat returns Ok(()) when non-interactive mode succeeds.
    let result = clawd::cli::chat::run_chat(opts, &config).await;
    assert!(result.is_ok(), "non-interactive failed: {result:?}");
}
