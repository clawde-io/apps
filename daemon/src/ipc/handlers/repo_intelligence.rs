/// RPC handlers for the Repo Intelligence subsystem (Sprint F, RI.T05–T18).
///
/// Registered methods:
///   repo.scan             — run full scanner on a registered repo
///   repo.profile          — return stored profile
///   repo.generateArtifacts — generate CLAUDE.md, AGENTS.md, .cursor/rules
///   repo.syncArtifacts    — propagate changes across artifacts
///   repo.driftScore       — compute 0–100 drift score
///   repo.driftReport      — list specific drift issues
///   validators.list       — list auto-derived validators for a repo
///   validators.run        — execute a validator and store the result
use crate::repo_intelligence::{artifacts, drift, scanner, storage, validator};
use crate::AppContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

// ─── Param structs ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RepoPathParams {
    #[serde(rename = "repoPath")]
    repo_path: String,
}

#[derive(Deserialize)]
struct GenerateArtifactsParams {
    #[serde(rename = "repoPath")]
    repo_path: String,
    /// If true, overwrite existing artifacts. Default: false (show diff only).
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
struct ValidatorRunParams {
    #[serde(rename = "repoPath")]
    repo_path: String,
    /// The validator command to run (must match one from validators.list).
    command: String,
}

// ─── Validation ───────────────────────────────────────────────────────────────

fn validate_repo_path(path: &str) -> Result<()> {
    if path.contains('\0') {
        bail!("invalid repoPath: null byte");
    }
    if !Path::new(path).is_absolute() {
        bail!("invalid repoPath: must be an absolute path");
    }
    if !Path::new(path).exists() {
        bail!("REPO_NOT_FOUND: path does not exist: {path}");
    }
    Ok(())
}

// ─── repo.scan ────────────────────────────────────────────────────────────────

/// `repo.scan` — run the full repo intelligence scanner and persist the profile.
///
/// The scan is synchronous I/O; it runs on a blocking thread pool to avoid
/// blocking the async executor.
pub async fn scan(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;

    let repo_path = p.repo_path.clone();
    let profile = tokio::task::spawn_blocking(move || scanner::scan(Path::new(&repo_path))).await?;

    // Persist to DB
    let pool = ctx.storage.pool();
    storage::upsert(&pool, &profile).await?;

    Ok(serde_json::to_value(&profile)?)
}

// ─── repo.profile ─────────────────────────────────────────────────────────────

/// `repo.profile` — return the stored profile for a repo, or null if not scanned.
pub async fn profile(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    let pool = ctx.storage.pool();
    match storage::load(&pool, &p.repo_path).await? {
        Some(profile) => Ok(serde_json::to_value(&profile)?),
        None => Ok(json!({ "repoPath": p.repo_path, "profile": null })),
    }
}

// ─── repo.generateArtifacts ───────────────────────────────────────────────────

/// `repo.generateArtifacts` — generate CLAUDE.md, AGENTS.md, and .cursor/rules
/// from the stored (or freshly computed) profile.
pub async fn generate_artifacts(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: GenerateArtifactsParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;

    // Load existing profile or scan fresh
    let pool = ctx.storage.pool();
    let profile = match storage::load(&pool, &p.repo_path).await? {
        Some(existing) => existing,
        None => {
            let repo_path = p.repo_path.clone();
            let scanned =
                tokio::task::spawn_blocking(move || scanner::scan(Path::new(&repo_path))).await?;
            storage::upsert(&pool, &scanned).await?;
            scanned
        }
    };

    let results = artifacts::generate_all(&profile, p.overwrite).await?;
    Ok(json!({ "artifacts": results }))
}

// ─── repo.syncArtifacts ──────────────────────────────────────────────────────

/// `repo.syncArtifacts` — propagate changes across CLAUDE.md / AGENTS.md / .cursor/rules.
///
/// Detects missing artifacts and regenerates them from the existing CLAUDE.md content.
pub async fn sync_artifacts(params: Value, _ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    let updated = artifacts::sync_artifacts(&p.repo_path).await?;
    Ok(json!({ "updated": updated }))
}

// ─── repo.driftScore ─────────────────────────────────────────────────────────

/// `repo.driftScore` — compute a 0–100 repo drift score.
pub async fn drift_score(params: Value, _ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    let repo_path = p.repo_path.clone();
    let score =
        tokio::task::spawn_blocking(move || drift::drift_score(Path::new(&repo_path))).await?;
    Ok(json!({ "repoPath": p.repo_path, "score": score }))
}

// ─── repo.driftReport ────────────────────────────────────────────────────────

/// `repo.driftReport` — return a list of repo drift issues.
pub async fn drift_report(params: Value, _ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    let repo_path = p.repo_path.clone();
    let items =
        tokio::task::spawn_blocking(move || drift::drift_report(Path::new(&repo_path))).await?;
    Ok(json!({ "repoPath": p.repo_path, "items": items }))
}

// ─── validators.list ─────────────────────────────────────────────────────────

/// `validators.list` — list auto-derived validators for a repo.
///
/// First checks for a stored profile; if none, runs a quick language detection.
pub async fn validators_list(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;

    let pool = ctx.storage.pool();
    let lang = match storage::load(&pool, &p.repo_path).await? {
        Some(profile) => profile.primary_lang,
        None => {
            let repo_path = p.repo_path.clone();
            tokio::task::spawn_blocking(move || {
                scanner::detect_primary_language(Path::new(&repo_path))
            })
            .await?
        }
    };

    let validators = validator::derive_validators(&lang);
    Ok(json!({ "repoPath": p.repo_path, "validators": validators }))
}

// ─── validators.run ──────────────────────────────────────────────────────────

/// `validators.run` — execute a validator command in the repo and store the result.
pub async fn validators_run(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ValidatorRunParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;

    // Verify the command matches a known validator (security guard)
    let pool = ctx.storage.pool();
    let lang = match storage::load(&pool, &p.repo_path).await? {
        Some(profile) => profile.primary_lang,
        None => {
            let repo_path = p.repo_path.clone();
            tokio::task::spawn_blocking(move || {
                scanner::detect_primary_language(Path::new(&repo_path))
            })
            .await?
        }
    };

    let known = validator::derive_validators(&lang);
    let config = known
        .into_iter()
        .find(|v| v.command == p.command)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unknown validator command '{}' — call validators.list to get valid commands",
                p.command
            )
        })?;

    let pool = ctx.storage.pool();
    let run = validator::run_validator(&pool, &p.repo_path, &config).await?;
    Ok(serde_json::to_value(&run)?)
}

// ─── Convention injection hook (RI.T08) ──────────────────────────────────────

/// Return a convention injection block for `session.create` system prompt.
///
/// Called by the session handler when a `repo_path` is provided. Returns `None`
/// if no profile exists or conventions are unknown.
pub async fn convention_injection(repo_path: &str, ctx: &AppContext) -> Option<String> {
    let pool = ctx.storage.pool();
    let profile = storage::load(&pool, repo_path).await.ok()??;
    let conv = &profile.conventions;

    let mut lines = Vec::new();
    lines.push(format!("Repo language: {}", profile.primary_lang.as_str()));
    if let Some(ref naming) = conv.naming_style {
        lines.push(format!("Naming convention: {naming}"));
    }
    if let Some(ref indent) = conv.indentation {
        lines.push(format!("Indentation: {indent}"));
    }
    if let Some(max_len) = conv.max_line_length {
        lines.push(format!("Approximate max line length: {max_len} characters"));
    }
    if !profile.frameworks.is_empty() {
        let fw: Vec<&str> = profile.frameworks.iter().map(|f| f.as_str()).collect();
        lines.push(format!("Detected frameworks: {}", fw.join(", ")));
    }

    if lines.len() <= 1 {
        return None;
    }
    Some(format!("## Repo conventions\n\n{}\n", lines.join("\n")))
}
