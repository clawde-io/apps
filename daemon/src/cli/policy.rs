// cli/policy.rs — `clawd policy test` CLI (Sprint ZZ PT.T02)
//
// Executes policy YAML test files against the live policy engine.
// Exit code 1 on any failure.

use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};

/// PT.T02 — `clawd policy test [--file <yaml>]`
pub async fn test(
    file: Option<PathBuf>,
    ci: bool,
    data_dir: &Path,
    port: u16,
) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let mut params = json!({ "ci": ci });
    if let Some(f) = file {
        params["file"] = json!(f.to_string_lossy());
    }

    let result = client.call_once("policy.test", params).await?;

    let total = result["total"].as_u64().unwrap_or(0);
    let passed = result["passed"].as_u64().unwrap_or(0);
    let failed = result["failed"].as_u64().unwrap_or(0);
    let cases = result["cases"].as_array().cloned().unwrap_or_default();

    // Print individual failures
    for case in &cases {
        let ok = case["passed"].as_bool().unwrap_or(false);
        if !ok {
            let command = case["command"].as_str().unwrap_or("?");
            let expected = case["expected"].as_str().unwrap_or("?");
            let actual = case["actual"].as_str().unwrap_or("?");
            let rule = case["triggered_rule"].as_str().unwrap_or("none");
            eprintln!(
                "FAIL  [{expected} expected, got {actual}] rule={rule}  \"{command}\""
            );
        }
    }

    if ci {
        println!(
            "policy-test: {total} total, {passed} passed, {failed} failed — {}",
            if failed == 0 { "PASS" } else { "FAIL" }
        );
    } else {
        println!(
            "\nPolicy test: {total} total, {passed} passed, {failed} failed — {}",
            if failed == 0 { "✓ PASS" } else { "✗ FAIL" }
        );
    }

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Install seed policy tests into `{project}/.clawd/tests/policy/`.
pub async fn install_seed_tests(project_path: &Path) -> Result<()> {
    let policy_dir = project_path.join(".clawd/tests/policy");
    tokio::fs::create_dir_all(&policy_dir).await?;

    let seed_path = policy_dir.join("seed.yaml");
    if !seed_path.exists() {
        tokio::fs::write(
            &seed_path,
            crate::policy::tester::SEED_POLICY_TESTS_YAML,
        )
        .await?;
        println!("Installed seed policy tests at {}", seed_path.display());
    } else {
        println!("Seed tests already exist at {}", seed_path.display());
    }

    Ok(())
}
