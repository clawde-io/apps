/// Integration tests for `clawd init` — AFS init + stack detection + templates.
///
/// Tests `claw_init::init_claw_dir`, `init_templates::detect_stack`, and
/// `init_templates::template_for` without spawning the full daemon.
use clawd::{claw_init, init_templates};
use std::fs;
use tempfile::TempDir;

// ─── init_claw_dir ────────────────────────────────────────────────────────────

#[tokio::test]
async fn init_creates_required_directories() {
    let dir = TempDir::new().unwrap();
    claw_init::init_claw_dir(dir.path()).await.unwrap();

    for sub in ["tasks", "policies", "templates", "evals/datasets", "worktrees"] {
        assert!(
            dir.path().join(".claw").join(sub).exists(),
            "missing directory: .claw/{sub}"
        );
    }
}

#[tokio::test]
async fn init_creates_policy_files() {
    let dir = TempDir::new().unwrap();
    claw_init::init_claw_dir(dir.path()).await.unwrap();

    let tool_risk = dir.path().join(".claw/policies/tool-risk.json");
    let mcp_trust = dir.path().join(".claw/policies/mcp-trust.json");

    assert!(tool_risk.exists(), "tool-risk.json missing");
    assert!(mcp_trust.exists(), "mcp-trust.json missing");

    // tool-risk.json must be valid JSON with expected keys.
    let tr: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&tool_risk).unwrap()).unwrap();
    assert!(tr.get("shell_exec").is_some(), "tool-risk must contain shell_exec");
    assert!(tr.get("read_file").is_some(), "tool-risk must contain read_file");

    // mcp-trust.json must be valid JSON with a "servers" array.
    let mt: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_trust).unwrap()).unwrap();
    assert!(
        mt.get("servers").is_some(),
        "mcp-trust.json must contain servers key"
    );
}

#[tokio::test]
async fn init_is_idempotent() {
    let dir = TempDir::new().unwrap();
    // First call.
    claw_init::init_claw_dir(dir.path()).await.unwrap();
    let first_tool_risk =
        fs::read_to_string(dir.path().join(".claw/policies/tool-risk.json")).unwrap();
    // Second call must not overwrite.
    claw_init::init_claw_dir(dir.path()).await.unwrap();
    let second_tool_risk =
        fs::read_to_string(dir.path().join(".claw/policies/tool-risk.json")).unwrap();
    assert_eq!(first_tool_risk, second_tool_risk, "idempotent: tool-risk.json unchanged");
}

#[tokio::test]
async fn validate_passes_after_init() {
    let dir = TempDir::new().unwrap();
    claw_init::init_claw_dir(dir.path()).await.unwrap();
    let missing = claw_init::validate_claw_dir(dir.path()).await;
    assert!(
        missing.is_empty(),
        "validate should pass after init, got missing: {missing:?}"
    );
}

#[tokio::test]
async fn validate_reports_missing_on_empty_dir() {
    let dir = TempDir::new().unwrap();
    let missing = claw_init::validate_claw_dir(dir.path()).await;
    assert!(!missing.is_empty(), "empty dir should have missing items");
}

// ─── detect_stack ─────────────────────────────────────────────────────────────

#[test]
fn detect_stack_rust() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    assert_eq!(
        init_templates::detect_stack(dir.path()),
        init_templates::Stack::RustCli
    );
}

#[test]
fn detect_stack_nextjs() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();
    fs::write(dir.path().join("next.config.js"), "module.exports = {}").unwrap();
    assert_eq!(
        init_templates::detect_stack(dir.path()),
        init_templates::Stack::NextJs
    );
}

#[test]
fn detect_stack_react_spa() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();
    fs::write(dir.path().join("vite.config.ts"), "export default {}").unwrap();
    assert_eq!(
        init_templates::detect_stack(dir.path()),
        init_templates::Stack::ReactSpa
    );
}

#[test]
fn detect_stack_flutter() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("pubspec.yaml"), "name: myapp").unwrap();
    assert_eq!(
        init_templates::detect_stack(dir.path()),
        init_templates::Stack::FlutterApp
    );
}

#[test]
fn detect_stack_nself() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".env.nself"), "# nself env").unwrap();
    assert_eq!(
        init_templates::detect_stack(dir.path()),
        init_templates::Stack::NselfBackend
    );
}

#[test]
fn detect_stack_generic_fallback() {
    let dir = TempDir::new().unwrap();
    // No marker files — should fall back to Generic.
    assert_eq!(
        init_templates::detect_stack(dir.path()),
        init_templates::Stack::Generic
    );
}

// ─── template_for ─────────────────────────────────────────────────────────────

#[test]
fn templates_have_content_for_all_stacks() {
    use init_templates::Stack;

    let stacks = [
        Stack::RustCli,
        Stack::NextJs,
        Stack::ReactSpa,
        Stack::FlutterApp,
        Stack::NselfBackend,
        Stack::Generic,
    ];

    for stack in stacks {
        let tmpl = init_templates::template_for(stack);
        assert!(
            !tmpl.claude_md.is_empty(),
            "{stack} CLAUDE.md template is empty"
        );
        assert!(
            !tmpl.decisions_md.is_empty(),
            "{stack} decisions.md template is empty"
        );
        // gitignore_additions can be empty for Generic — that's fine.
    }
}

#[test]
fn rust_template_mentions_clippy() {
    let tmpl = init_templates::template_for(init_templates::Stack::RustCli);
    assert!(
        tmpl.claude_md.contains("clippy"),
        "Rust template must mention clippy"
    );
}

#[test]
fn nextjs_template_mentions_pnpm() {
    let tmpl = init_templates::template_for(init_templates::Stack::NextJs);
    assert!(
        tmpl.claude_md.contains("pnpm"),
        "Next.js template must mention pnpm"
    );
}

#[test]
fn flutter_template_mentions_riverpod() {
    let tmpl = init_templates::template_for(init_templates::Stack::FlutterApp);
    assert!(
        tmpl.claude_md.contains("Riverpod"),
        "Flutter template must mention Riverpod"
    );
}
