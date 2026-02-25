/// Validator engine — auto-derive and run language-appropriate linters/test suites
/// from a StackProfile (RI.T17–T18).
use super::profile::PrimaryLanguage;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// A single validator command (linter, test runner, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorConfig {
    /// Short display name, e.g. "cargo clippy"
    pub name: String,
    /// The shell command to run (run in the repo root)
    pub command: String,
    /// Brief human-readable description
    pub description: String,
    /// Whether this validator is auto-derived (vs user-defined)
    pub auto_derived: bool,
}

/// Result of a validator run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorRun {
    pub id: String,
    pub repo_path: String,
    pub validator_cmd: String,
    pub exit_code: Option<i64>,
    pub output: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

// ─── Auto-derivation (RI.T17) ─────────────────────────────────────────────────

/// Derive validator commands from a detected primary language.
///
/// Returns a list of validators to run — caller should execute them in order.
pub fn derive_validators(lang: &PrimaryLanguage) -> Vec<ValidatorConfig> {
    match lang {
        PrimaryLanguage::Rust => vec![
            ValidatorConfig {
                name: "cargo clippy".to_string(),
                command: "cargo clippy --all-targets --all-features -- -D warnings".to_string(),
                description: "Lint: run Clippy on all targets".to_string(),
                auto_derived: true,
            },
            ValidatorConfig {
                name: "cargo test".to_string(),
                command: "cargo test --all-targets".to_string(),
                description: "Tests: run the full test suite".to_string(),
                auto_derived: true,
            },
        ],
        PrimaryLanguage::TypeScript => vec![
            ValidatorConfig {
                name: "tsc --noEmit".to_string(),
                command: "npx tsc --noEmit".to_string(),
                description: "Type-check: verify TypeScript compilation".to_string(),
                auto_derived: true,
            },
            ValidatorConfig {
                name: "eslint".to_string(),
                command: "npx eslint . --ext .ts,.tsx".to_string(),
                description: "Lint: run ESLint on TypeScript files".to_string(),
                auto_derived: true,
            },
        ],
        PrimaryLanguage::JavaScript => vec![ValidatorConfig {
            name: "eslint".to_string(),
            command: "npx eslint . --ext .js,.jsx,.mjs".to_string(),
            description: "Lint: run ESLint".to_string(),
            auto_derived: true,
        }],
        PrimaryLanguage::Dart => vec![
            ValidatorConfig {
                name: "flutter analyze".to_string(),
                command: "flutter analyze".to_string(),
                description: "Lint: run Flutter/Dart analyzer".to_string(),
                auto_derived: true,
            },
            ValidatorConfig {
                name: "flutter test".to_string(),
                command: "flutter test".to_string(),
                description: "Tests: run Flutter test suite".to_string(),
                auto_derived: true,
            },
        ],
        PrimaryLanguage::Python => vec![
            ValidatorConfig {
                name: "ruff check".to_string(),
                command: "ruff check .".to_string(),
                description: "Lint: run Ruff linter".to_string(),
                auto_derived: true,
            },
            ValidatorConfig {
                name: "pytest".to_string(),
                command: "pytest".to_string(),
                description: "Tests: run pytest".to_string(),
                auto_derived: true,
            },
        ],
        PrimaryLanguage::Go => vec![
            ValidatorConfig {
                name: "go vet".to_string(),
                command: "go vet ./...".to_string(),
                description: "Lint: run go vet".to_string(),
                auto_derived: true,
            },
            ValidatorConfig {
                name: "go test".to_string(),
                command: "go test ./...".to_string(),
                description: "Tests: run go test suite".to_string(),
                auto_derived: true,
            },
        ],
        PrimaryLanguage::Ruby => vec![ValidatorConfig {
            name: "rubocop".to_string(),
            command: "rubocop".to_string(),
            description: "Lint: run RuboCop".to_string(),
            auto_derived: true,
        }],
        PrimaryLanguage::Swift => vec![ValidatorConfig {
            name: "swift build".to_string(),
            command: "swift build".to_string(),
            description: "Build: compile Swift package".to_string(),
            auto_derived: true,
        }],
        PrimaryLanguage::Kotlin | PrimaryLanguage::Java => vec![ValidatorConfig {
            name: "gradle lint".to_string(),
            command: "./gradlew lint".to_string(),
            description: "Lint: run Gradle lint".to_string(),
            auto_derived: true,
        }],
        PrimaryLanguage::Unknown => vec![],
    }
}

// ─── Storage helpers (RI.T18) ─────────────────────────────────────────────────

/// Persist a validator run record to the database and return the run ID.
pub async fn record_run_start(
    pool: &SqlitePool,
    repo_path: &str,
    validator_cmd: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO validator_runs (id, repo_path, validator_cmd, started_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(repo_path)
    .bind(validator_cmd)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Update a completed validator run with exit code and output.
pub async fn record_run_finish(
    pool: &SqlitePool,
    id: &str,
    exit_code: i64,
    output: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE validator_runs SET exit_code = ?, output = ?, finished_at = ? WHERE id = ?",
    )
    .bind(exit_code)
    .bind(output)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// List recent validator runs for a repository (last 20).
pub async fn list_runs(pool: &SqlitePool, repo_path: &str) -> Result<Vec<ValidatorRun>> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<i64>, Option<String>, String, Option<String>)>(
        "SELECT id, repo_path, validator_cmd, exit_code, output, started_at, finished_at
         FROM validator_runs WHERE repo_path = ? ORDER BY started_at DESC LIMIT 20",
    )
    .bind(repo_path)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, repo_path, validator_cmd, exit_code, output, started_at, finished_at)| {
                ValidatorRun {
                    id,
                    repo_path,
                    validator_cmd,
                    exit_code,
                    output,
                    started_at,
                    finished_at,
                }
            },
        )
        .collect())
}

/// Run a validator command in a subprocess and store the result.
///
/// Returns the finished `ValidatorRun`. The command is run with a 5-minute timeout.
pub async fn run_validator(
    pool: &SqlitePool,
    repo_path: &str,
    config: &ValidatorConfig,
) -> Result<ValidatorRun> {
    let run_id = record_run_start(pool, repo_path, &config.command).await?;

    let parts: Vec<&str> = config.command.split_whitespace().collect();
    let (cmd, args) = parts.split_first().unwrap_or((&"echo", &[]));

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::process::Command::new(cmd)
            .args(args)
            .current_dir(repo_path)
            .output(),
    )
    .await;

    let (exit_code, output) = match result {
        Ok(Ok(out)) => {
            let code = out.status.code().unwrap_or(-1) as i64;
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.is_empty() {
                combined.push('\n');
                combined.push_str(&stderr);
            }
            (code, combined)
        }
        Ok(Err(e)) => (-1i64, format!("spawn error: {e}")),
        Err(_) => (-1i64, "timeout after 300s".to_string()),
    };

    record_run_finish(pool, &run_id, exit_code, &output).await?;

    Ok(ValidatorRun {
        id: run_id,
        repo_path: repo_path.to_string(),
        validator_cmd: config.command.clone(),
        exit_code: Some(exit_code),
        output: Some(output),
        started_at: Utc::now().to_rfc3339(),
        finished_at: Some(Utc::now().to_rfc3339()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_has_two_validators() {
        let validators = derive_validators(&PrimaryLanguage::Rust);
        assert_eq!(validators.len(), 2);
        assert!(validators.iter().any(|v| v.name == "cargo clippy"));
        assert!(validators.iter().any(|v| v.name == "cargo test"));
    }

    #[test]
    fn dart_has_two_validators() {
        let validators = derive_validators(&PrimaryLanguage::Dart);
        assert_eq!(validators.len(), 2);
        assert!(validators.iter().any(|v| v.name == "flutter analyze"));
    }

    #[test]
    fn unknown_has_no_validators() {
        let validators = derive_validators(&PrimaryLanguage::Unknown);
        assert!(validators.is_empty());
    }

    #[test]
    fn all_validators_are_auto_derived() {
        for lang in &[
            PrimaryLanguage::Rust,
            PrimaryLanguage::TypeScript,
            PrimaryLanguage::Python,
            PrimaryLanguage::Go,
        ] {
            for v in derive_validators(lang) {
                assert!(
                    v.auto_derived,
                    "Expected auto_derived=true for {lang:?}/{name}",
                    name = v.name
                );
            }
        }
    }
}
