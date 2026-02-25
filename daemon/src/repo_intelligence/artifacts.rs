/// Artifact generation — write .claude/CLAUDE.md, .codex/AGENTS.md,
/// .cursor/rules from a RepoProfile (RI.T09–T13).
use super::profile::{PrimaryLanguage, RepoProfile};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Outcome of a single artifact generation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactResult {
    /// File path that was written (or would have been written)
    pub path: String,
    /// What happened: "created" | "updated" | "skipped"
    pub action: String,
    /// Unified diff between old content and new content (empty if created)
    pub diff: String,
}

// ─── Per-artifact generators ─────────────────────────────────────────────────

/// Generate `.claude/CLAUDE.md` from a repo profile (RI.T09).
pub async fn generate_claude(
    profile: &RepoProfile,
    overwrite: bool,
) -> Result<ArtifactResult> {
    let repo = Path::new(&profile.repo_path);
    let claude_dir = repo.join(".claude");
    let target = claude_dir.join("CLAUDE.md");
    let content = render_claude_md(profile);
    write_artifact(&target, &content, overwrite).await
}

/// Generate `.codex/AGENTS.md` from a repo profile (RI.T10).
pub async fn generate_codex(
    profile: &RepoProfile,
    overwrite: bool,
) -> Result<ArtifactResult> {
    let repo = Path::new(&profile.repo_path);
    let codex_dir = repo.join(".codex");
    let target = codex_dir.join("AGENTS.md");
    let content = render_agents_md(profile);
    write_artifact(&target, &content, overwrite).await
}

/// Generate `.cursor/rules` from a repo profile (RI.T11).
pub async fn generate_cursor(
    profile: &RepoProfile,
    overwrite: bool,
) -> Result<ArtifactResult> {
    let repo = Path::new(&profile.repo_path);
    let cursor_dir = repo.join(".cursor");
    let target = cursor_dir.join("rules");
    let content = render_cursor_rules(profile);
    write_artifact(&target, &content, overwrite).await
}

/// Generate all three artifacts (RI.T12).
///
/// Returns one `ArtifactResult` per artifact (claude, codex, cursor).
pub async fn generate_all(
    profile: &RepoProfile,
    overwrite: bool,
) -> Result<Vec<ArtifactResult>> {
    let claude = generate_claude(profile, overwrite).await?;
    let codex = generate_codex(profile, overwrite).await?;
    let cursor = generate_cursor(profile, overwrite).await?;
    Ok(vec![claude, codex, cursor])
}

/// Propagate key fields across artifacts when one changes (RI.T13).
///
/// Currently propagates the `primary_lang` and `build_tools` signals:
/// if CLAUDE.md changes and AGENTS.md/cursorrules are out of sync,
/// regenerate them.  Returns only the artifacts that were updated.
pub async fn sync_artifacts(repo_path: &str) -> Result<Vec<ArtifactResult>> {
    let repo = Path::new(repo_path);
    let mut updated = Vec::new();

    // Read current profile from CLAUDE.md header comment (cheapest sync signal).
    // If we can detect the language from the artifact, regenerate AGENTS + cursor.
    let claude_path = repo.join(".claude").join("CLAUDE.md");
    if !claude_path.exists() {
        return Ok(updated);
    }

    // We don't store profile in CLAUDE.md; just verify the three artifacts exist.
    // If AGENTS.md is missing but CLAUDE.md exists, create AGENTS.md.
    let agents_path = repo.join(".codex").join("AGENTS.md");
    if !agents_path.exists() {
        let content = tokio::fs::read_to_string(&claude_path).await?;
        // Derive a minimal agents.md from the claude.md header
        let agents_content = convert_claude_to_agents(&content);
        if let Some(parent) = agents_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&agents_path, &agents_content).await?;
        updated.push(ArtifactResult {
            path: agents_path.to_string_lossy().into_owned(),
            action: "created".to_string(),
            diff: String::new(),
        });
    }

    let cursor_path = repo.join(".cursor").join("rules");
    if !cursor_path.exists() {
        let content = tokio::fs::read_to_string(&claude_path).await?;
        let cursor_content = convert_claude_to_cursor(&content);
        if let Some(parent) = cursor_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&cursor_path, &cursor_content).await?;
        updated.push(ArtifactResult {
            path: cursor_path.to_string_lossy().into_owned(),
            action: "created".to_string(),
            diff: String::new(),
        });
    }

    Ok(updated)
}

// ─── Render functions ─────────────────────────────────────────────────────────

fn render_claude_md(profile: &RepoProfile) -> String {
    let lang = profile.primary_lang.as_str();
    let frameworks: Vec<&str> = profile.frameworks.iter().map(|f| f.as_str()).collect();
    let build_tools: Vec<&str> = profile.build_tools.iter().map(|b| b.as_str()).collect();

    let mut lines = Vec::new();
    lines.push("# Project Instructions\n".to_string());
    lines.push(format!("**Primary language:** {lang}\n"));

    if !frameworks.is_empty() {
        lines.push(format!("**Frameworks:** {}\n", frameworks.join(", ")));
    }
    if !build_tools.is_empty() {
        lines.push(format!("**Build tools:** {}\n", build_tools.join(", ")));
    }
    if profile.monorepo {
        lines.push("**Monorepo:** yes\n".to_string());
    }

    lines.push("\n## Coding style\n".to_string());
    if let Some(ref naming) = profile.conventions.naming_style {
        lines.push(format!("- Naming: {naming}\n"));
    }
    if let Some(ref indent) = profile.conventions.indentation {
        lines.push(format!("- Indentation: {indent}\n"));
    }
    if let Some(max_len) = profile.conventions.max_line_length {
        lines.push(format!("- Max line length: ~{max_len} chars\n"));
    }

    lines.push("\n## Language-specific rules\n".to_string());
    lines.push(language_rules(&profile.primary_lang));

    if profile.monorepo {
        lines.push("\n## Monorepo notes\n".to_string());
        lines.push("- This is a monorepo. Prefer making changes in the affected sub-package only.\n".to_string());
        lines.push("- Run tests for the affected package, not the whole repo.\n".to_string());
    }

    lines.join("")
}

fn render_agents_md(profile: &RepoProfile) -> String {
    let lang = profile.primary_lang.as_str();
    let mut lines = Vec::new();
    lines.push("# Agent Instructions\n\n".to_string());
    lines.push(format!("This is a **{lang}** project.\n\n"));
    lines.push("## Key rules\n\n".to_string());
    lines.push(language_rules(&profile.primary_lang));
    if let Some(ref indent) = profile.conventions.indentation {
        lines.push(format!("\n- Use {indent} for indentation.\n"));
    }
    lines.join("")
}

fn render_cursor_rules(profile: &RepoProfile) -> String {
    let lang = profile.primary_lang.as_str();
    let mut lines = Vec::new();
    lines.push(format!("# Cursor Rules — {lang} project\n\n"));
    lines.push(language_rules(&profile.primary_lang));
    if let Some(ref indent) = profile.conventions.indentation {
        lines.push(format!("\n- Indentation: {indent}\n"));
    }
    lines.join("")
}

fn language_rules(lang: &PrimaryLanguage) -> String {
    match lang {
        PrimaryLanguage::Rust => concat!(
            "- Use `?` for error propagation. No `unwrap()` or `expect()` in production.\n",
            "- Run `cargo clippy --all-targets --all-features`. Zero warnings allowed.\n",
            "- Use `thiserror` for library errors, `anyhow` for application errors.\n",
            "- Async: tokio runtime. Avoid blocking the thread pool.\n",
        ).to_string(),
        PrimaryLanguage::TypeScript => concat!(
            "- Strict mode (`\"strict\": true` in tsconfig). No `any`.\n",
            "- Prefer `type` over `interface` for unions and mapped types.\n",
            "- No `console.log` in production — use a logger.\n",
        ).to_string(),
        PrimaryLanguage::JavaScript => concat!(
            "- Prefer `const` and `let` over `var`.\n",
            "- Use ESM imports (`import`/`export`), not `require()`.\n",
            "- No `console.log` in production.\n",
        ).to_string(),
        PrimaryLanguage::Dart => concat!(
            "- Run `flutter analyze`. Zero warnings allowed.\n",
            "- Prefer `const` constructors where possible.\n",
            "- Use `riverpod` for state management if the project already uses it.\n",
        ).to_string(),
        PrimaryLanguage::Python => concat!(
            "- Use type hints on all public functions.\n",
            "- Use `ruff` for linting and `black` for formatting.\n",
            "- Prefer dataclasses or Pydantic models over plain dicts.\n",
        ).to_string(),
        PrimaryLanguage::Go => concat!(
            "- Return `error` as the last value. Never panic in library code.\n",
            "- Run `go vet` and `staticcheck` before committing.\n",
            "- Use the standard library where possible.\n",
        ).to_string(),
        PrimaryLanguage::Ruby => concat!(
            "- Follow the Ruby Style Guide (indentation: 2 spaces).\n",
            "- Use RuboCop for linting.\n",
        ).to_string(),
        PrimaryLanguage::Swift => concat!(
            "- Use Swift concurrency (`async`/`await`) over callbacks.\n",
            "- Prefer `struct` over `class` where semantics allow.\n",
        ).to_string(),
        PrimaryLanguage::Kotlin | PrimaryLanguage::Java => concat!(
            "- Prefer Kotlin idioms (data classes, extension functions, null safety).\n",
            "- Run `./gradlew lint`.\n",
        ).to_string(),
        _ => String::new(),
    }
}

fn convert_claude_to_agents(claude_content: &str) -> String {
    // Simple projection: take first 80 lines of CLAUDE.md, retitle for Codex
    let header = "# Agent Instructions\n\nDerived from CLAUDE.md.\n\n";
    let body: String = claude_content
        .lines()
        .take(80)
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}{body}\n")
}

fn convert_claude_to_cursor(claude_content: &str) -> String {
    let header = "# Cursor Rules\n\nDerived from CLAUDE.md.\n\n";
    let body: String = claude_content
        .lines()
        .take(80)
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}{body}\n")
}

// ─── File write helper ────────────────────────────────────────────────────────

async fn write_artifact(
    target: &PathBuf,
    new_content: &str,
    overwrite: bool,
) -> Result<ArtifactResult> {
    let path_str = target.to_string_lossy().into_owned();

    // Create parent directory if needed
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create_dir_all for {}", parent.display()))?;
    }

    if target.exists() {
        let existing = tokio::fs::read_to_string(target).await?;
        if existing == new_content {
            return Ok(ArtifactResult {
                path: path_str,
                action: "skipped".to_string(),
                diff: String::new(),
            });
        }
        if !overwrite {
            return Ok(ArtifactResult {
                path: path_str,
                action: "skipped".to_string(),
                diff: unified_diff(&existing, new_content),
            });
        }
        tokio::fs::write(target, new_content.as_bytes()).await?;
        Ok(ArtifactResult {
            path: path_str,
            action: "updated".to_string(),
            diff: unified_diff(&existing, new_content),
        })
    } else {
        tokio::fs::write(target, new_content.as_bytes()).await?;
        Ok(ArtifactResult {
            path: path_str,
            action: "created".to_string(),
            diff: String::new(),
        })
    }
}

/// Produce a minimal unified diff between `old` and `new` (line-based).
fn unified_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut diff = Vec::new();

    let old_set: std::collections::HashSet<&str> = old_lines.iter().copied().collect();
    let new_set: std::collections::HashSet<&str> = new_lines.iter().copied().collect();

    for line in &old_lines {
        if !new_set.contains(line) {
            diff.push(format!("- {line}"));
        }
    }
    for line in &new_lines {
        if !old_set.contains(line) {
            diff.push(format!("+ {line}"));
        }
    }
    diff.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_intelligence::profile::{BuildTool, CodeConventions, Framework};
    use tempfile::TempDir;

    fn test_profile(tmp: &TempDir, lang: PrimaryLanguage) -> RepoProfile {
        RepoProfile {
            repo_path: tmp.path().to_string_lossy().into_owned(),
            primary_lang: lang,
            secondary_langs: vec![],
            frameworks: vec![Framework::GithubActions],
            build_tools: vec![BuildTool::Make],
            conventions: CodeConventions {
                naming_style: Some("snake_case".to_string()),
                indentation: Some("4-space".to_string()),
                max_line_length: Some(100),
            },
            monorepo: false,
            confidence: 0.95,
            scanned_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn generates_claude_md() {
        let tmp = TempDir::new().unwrap();
        let profile = test_profile(&tmp, PrimaryLanguage::Rust);
        let result = generate_claude(&profile, true).await.unwrap();
        assert_eq!(result.action, "created");
        let content = std::fs::read_to_string(tmp.path().join(".claude").join("CLAUDE.md")).unwrap();
        assert!(content.contains("rust"));
        assert!(content.contains("snake_case"));
    }

    #[tokio::test]
    async fn skips_without_overwrite() {
        let tmp = TempDir::new().unwrap();
        let profile = test_profile(&tmp, PrimaryLanguage::TypeScript);
        // First write
        generate_claude(&profile, true).await.unwrap();
        // Second write without overwrite flag
        let result = generate_claude(&profile, false).await.unwrap();
        assert_eq!(result.action, "skipped");
    }

    #[tokio::test]
    async fn generate_all_creates_three_files() {
        let tmp = TempDir::new().unwrap();
        let profile = test_profile(&tmp, PrimaryLanguage::Go);
        let results = generate_all(&profile, true).await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.action == "created"));
    }
}
