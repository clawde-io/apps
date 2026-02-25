/// Persistence helpers for `repo_profiles` table (migration 018).
use super::profile::{BuildTool, CodeConventions, Framework, PrimaryLanguage, RepoProfile};
use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;

/// Row type for `repo_profiles` SELECT queries.
#[derive(sqlx::FromRow)]
struct RepoProfileRow {
    primary_lang: String,
    secondary_langs: String,
    frameworks: String,
    build_tools: String,
    conventions: String,
    monorepo: i64,
    confidence: f64,
    scanned_at: String,
}

/// Upsert a `RepoProfile` into the `repo_profiles` table.
pub async fn upsert(pool: &SqlitePool, profile: &RepoProfile) -> Result<()> {
    let primary_lang = profile.primary_lang.as_str().to_string();
    let secondary_langs = serde_json::to_string(&profile.secondary_langs)?;
    let frameworks = serde_json::to_string(&profile.frameworks)?;
    let build_tools = serde_json::to_string(&profile.build_tools)?;
    let conventions = serde_json::to_string(&profile.conventions)?;
    let monorepo = if profile.monorepo { 1i64 } else { 0i64 };
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO repo_profiles
             (repo_path, primary_lang, secondary_langs, frameworks, build_tools,
              conventions, monorepo, confidence, scanned_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(repo_path) DO UPDATE SET
             primary_lang    = excluded.primary_lang,
             secondary_langs = excluded.secondary_langs,
             frameworks      = excluded.frameworks,
             build_tools     = excluded.build_tools,
             conventions     = excluded.conventions,
             monorepo        = excluded.monorepo,
             confidence      = excluded.confidence,
             scanned_at      = excluded.scanned_at,
             updated_at      = excluded.updated_at",
    )
    .bind(&profile.repo_path)
    .bind(&primary_lang)
    .bind(&secondary_langs)
    .bind(&frameworks)
    .bind(&build_tools)
    .bind(&conventions)
    .bind(monorepo)
    .bind(profile.confidence as f64)
    .bind(&profile.scanned_at)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

/// Load a stored `RepoProfile` for the given `repo_path`.
///
/// Returns `None` if the repo has not been scanned yet.
pub async fn load(pool: &SqlitePool, repo_path: &str) -> Result<Option<RepoProfile>> {
    let row: Option<RepoProfileRow> = sqlx::query_as(
        "SELECT primary_lang, secondary_langs, frameworks, build_tools,
                conventions, monorepo, confidence, scanned_at
         FROM repo_profiles WHERE repo_path = ?",
    )
    .bind(repo_path)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let primary_lang = row.primary_lang;
    let secondary_langs_json = row.secondary_langs;
    let frameworks_json = row.frameworks;
    let build_tools_json = row.build_tools;
    let conventions_json = row.conventions;
    let monorepo = row.monorepo;
    let confidence = row.confidence;
    let scanned_at = row.scanned_at;

    let primary_lang = PrimaryLanguage::from_tag(&primary_lang);
    let secondary_langs: Vec<PrimaryLanguage> =
        serde_json::from_str(&secondary_langs_json).unwrap_or_default();
    let frameworks: Vec<Framework> =
        serde_json::from_str(&frameworks_json).unwrap_or_default();
    let build_tools: Vec<BuildTool> =
        serde_json::from_str(&build_tools_json).unwrap_or_default();
    let conventions: CodeConventions =
        serde_json::from_str(&conventions_json).unwrap_or_default();

    Ok(Some(RepoProfile {
        repo_path: repo_path.to_string(),
        primary_lang,
        secondary_langs,
        frameworks,
        build_tools,
        conventions,
        monorepo: monorepo != 0,
        confidence: confidence as f32,
        scanned_at,
    }))
}
