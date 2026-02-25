// SPDX-License-Identifier: MIT
//! GCI/AID generation engine (PO.T07–PO.T12).
//!
//! Generates personalised `~/.claude/CLAUDE.md`, `.codex/AGENTS.md`, and
//! `~/.cursor/rules` from 7-question questionnaire answers.
//!
//! Template library: 5 base personas — solo-dev, team-lead, backend-focus,
//! full-stack, mobile-focus. Each maps to a different generated header, tone,
//! and default rule set.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info};

/// The 7-question questionnaire that drives personalised GCI generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionnaireAnswers {
    /// Primary programming languages, e.g. ["rust", "typescript"]
    pub primary_languages: Vec<String>,
    /// Project type tags, e.g. ["backend-api", "cli-tool"]
    pub project_types: Vec<String>,
    /// "solo", "small" (2–5), "medium" (6–20), "large" (20+)
    pub team_size: String,
    /// "supervised" | "balanced" | "autonomous"
    pub autonomy_level: String,
    /// "strict" | "relaxed"
    pub style_rules: String,
    /// "pr-based" | "trunk"
    pub git_workflow: String,
    /// Free-form AI pain-point tags, e.g. ["hallucinations", "context-loss"]
    pub pain_points: Vec<String>,
}

/// Generated output paths and content.
#[derive(Debug, Clone)]
pub struct GeneratedConfig {
    pub path: PathBuf,
    pub content: String,
    /// True if a backup was made (existing file was present).
    pub backed_up: bool,
    /// Path to the backup file, if created.
    pub backup_path: Option<PathBuf>,
}

/// GCI generator: builds personalised CLAUDE.md and companion files.
pub struct GciGenerator;

impl GciGenerator {
    /// Generate personalised `~/.claude/CLAUDE.md` from questionnaire answers.
    ///
    /// If the file already exists it is backed up to
    /// `~/.claude/CLAUDE.md.backup.{timestamp}` before being replaced.
    pub async fn generate_claude_md(answers: &QuestionnaireAnswers) -> Result<GeneratedConfig> {
        let path = claude_md_path().context("cannot determine home directory")?;
        let content = render_claude_md(answers);
        write_with_backup(path, content).await
    }

    /// Generate `.codex/AGENTS.md` in the current working directory.
    ///
    /// The file is shorter than CLAUDE.md — task-focused with no global mode system.
    pub async fn generate_codex_md(answers: &QuestionnaireAnswers) -> Result<GeneratedConfig> {
        let path = codex_agents_path().context("cannot determine home directory")?;
        let content = render_codex_md(answers);
        write_with_backup(path, content).await
    }

    /// Generate Cursor rules at `~/.cursor/rules` (global Cursor rules file).
    pub async fn generate_cursor_rules(answers: &QuestionnaireAnswers) -> Result<GeneratedConfig> {
        let path = cursor_rules_path().context("cannot determine home directory")?;
        let content = render_cursor_rules(answers);
        write_with_backup(path, content).await
    }
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

fn home() -> Option<PathBuf> {
    #[allow(deprecated)]
    std::env::home_dir()
}

fn claude_md_path() -> Option<PathBuf> {
    Some(home()?.join(".claude").join("CLAUDE.md"))
}

fn codex_agents_path() -> Option<PathBuf> {
    // Write global Codex AGENTS.md to ~/.codex/AGENTS.md
    Some(home()?.join(".codex").join("AGENTS.md"))
}

fn cursor_rules_path() -> Option<PathBuf> {
    // Cursor stores global rules at ~/.cursor/rules (plain text, no extension)
    Some(home()?.join(".cursor").join("rules"))
}

// ─── Template selection ───────────────────────────────────────────────────────

/// Infer the best-fit persona template from questionnaire answers.
fn select_template(answers: &QuestionnaireAnswers) -> &'static str {
    let langs = answers.primary_languages.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let types = answers.project_types.iter().map(|s| s.as_str()).collect::<Vec<_>>();

    let is_mobile = langs.contains(&"dart") || langs.contains(&"swift") || langs.contains(&"kotlin")
        || types.contains(&"mobile-app");
    let is_backend = langs.contains(&"rust") || langs.contains(&"go") || langs.contains(&"python")
        || types.contains(&"backend-api") || types.contains(&"cli-tool");
    let is_full_stack = (langs.contains(&"typescript") || langs.contains(&"javascript"))
        && is_backend;
    let is_team_lead = matches!(
        answers.team_size.as_str(),
        "medium" | "large"
    );

    if is_mobile {
        "mobile-focus"
    } else if is_team_lead {
        "team-lead"
    } else if is_full_stack {
        "full-stack"
    } else if is_backend {
        "backend-focus"
    } else {
        "solo-dev"
    }
}

// ─── CLAUDE.md renderer ───────────────────────────────────────────────────────

fn render_claude_md(answers: &QuestionnaireAnswers) -> String {
    let template = select_template(answers);
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let languages = answers.primary_languages.join(", ");
    let pain_points = if answers.pain_points.is_empty() {
        "general AI drift and context loss".to_string()
    } else {
        answers.pain_points.join(", ")
    };

    let autonomy_section = render_autonomy_section(&answers.autonomy_level);
    let style_section = render_style_section(&answers.style_rules);
    let git_section = render_git_section(&answers.git_workflow);
    let template_header = render_template_header(template, &answers.team_size);
    let language_rules = render_language_rules(&answers.primary_languages);
    let pain_point_rules = render_pain_point_rules(&answers.pain_points);

    format!(
        r#"# Global Claude Instructions

> Generated by ClawDE provider onboarding wizard — {date}
> Template: {template} | Languages: {languages}
> To regenerate: Settings → Provider Setup → Re-run Wizard

{template_header}

---

## Core Operating Principles

You are an expert software engineer assistant. You help me write clean, correct,
maintainable code. You do not add unsolicited features or documentation.
You ask for clarification rather than guessing at ambiguous requirements.

### Task Discipline
- Do exactly what is asked — nothing more, nothing less
- Always verify your understanding before starting complex tasks
- Flag contradictions and ambiguities before proceeding
- Never mark work done without verifying it compiles and tests pass

{autonomy_section}

---

## Languages & Frameworks

Primary: {languages}

{language_rules}

---

## Code Style

{style_section}

---

## Git Workflow

{git_section}

---

## Known Pain Points

Primary AI issues to guard against: {pain_points}

{pain_point_rules}

---

## Anti-Hallucination Rules

1. Never invent APIs, library functions, or file paths
2. If unsure about an API, say so — do not guess
3. Check that imports resolve before writing them
4. Never stub implementations — all functions must be complete
5. Verify filenames, types, and module paths against the actual codebase
"#
    )
}

fn render_template_header(template: &str, team_size: &str) -> String {
    match template {
        "solo-dev" => format!(
            "## Context: Solo Developer\n\
             \n\
             Working alone. Prioritise speed and clarity. Skip ceremony where it slows\n\
             things down. Tests matter but not at the cost of forward momentum.\n\
             Team size: {team_size}"
        ),
        "team-lead" => format!(
            "## Context: Team Lead\n\
             \n\
             Leading a team of engineers. Code must be exemplary — others learn from it.\n\
             Prioritise correctness, documentation, and patterns that scale.\n\
             Team size: {team_size} — document decisions, not just implementations."
        ),
        "backend-focus" => format!(
            "## Context: Backend Engineer\n\
             \n\
             Systems-oriented. Correctness and reliability over cleverness.\n\
             Error handling must be explicit. No panics in production. No silent failures.\n\
             Team size: {team_size}"
        ),
        "full-stack" => format!(
            "## Context: Full-Stack Developer\n\
             \n\
             Works across the entire stack. Consistency matters — use the same patterns\n\
             on frontend and backend where possible. Type safety end-to-end.\n\
             Team size: {team_size}"
        ),
        "mobile-focus" => format!(
            "## Context: Mobile Developer\n\
             \n\
             Mobile-first. Performance and battery life matter. Platform guidelines are\n\
             non-negotiable. Accessibility is required, not optional.\n\
             Team size: {team_size}"
        ),
        _ => format!("## Context\n\nTeam size: {team_size}"),
    }
}

fn render_autonomy_section(level: &str) -> String {
    match level {
        "supervised" => {
            "### Autonomy: Supervised\n\
             \n\
             - Always confirm before making structural changes\n\
             - Show a plan and wait for approval before executing multi-file changes\n\
             - Ask before installing new dependencies\n\
             - Prefer smaller, reviewable chunks over large sweeping changes"
                .to_string()
        }
        "balanced" => {
            "### Autonomy: Balanced\n\
             \n\
             - Proceed confidently on clear tasks within defined scope\n\
             - Confirm before structural changes (new modules, schema changes, dependency adds)\n\
             - Self-correct small errors without asking\n\
             - Report what you changed after significant edits"
                .to_string()
        }
        "autonomous" => {
            "### Autonomy: Autonomous\n\
             \n\
             - Execute tasks fully without intermediate check-ins\n\
             - Make reasonable decisions about implementation details\n\
             - Only pause for: missing credentials, destructive actions, or ambiguity that\n\
               would waste significant effort if wrong\n\
             - Summarise decisions made at the end of each task"
                .to_string()
        }
        _ => String::new(),
    }
}

fn render_style_section(style: &str) -> String {
    match style {
        "strict" => {
            "Style enforcement: **Strict**\n\
             \n\
             - No deviations from established conventions in this codebase\n\
             - Point out style violations in code I write too\n\
             - Linter must be clean — no suppressions without explanation\n\
             - Line length, naming, and formatting must match existing code exactly"
                .to_string()
        }
        "relaxed" => {
            "Style enforcement: **Relaxed**\n\
             \n\
             - Follow existing patterns but minor deviations are acceptable\n\
             - Prioritise readability over rigid convention adherence\n\
             - Flag obvious style issues but do not block on minor inconsistencies"
                .to_string()
        }
        _ => String::new(),
    }
}

fn render_git_section(workflow: &str) -> String {
    match workflow {
        "pr-based" => {
            "Git workflow: **PR-based**\n\
             \n\
             - All changes go through pull requests — never push directly to main\n\
             - Each PR should be focused and reviewable in under 30 minutes\n\
             - Write clear PR descriptions: what changed and why\n\
             - Squash fixup commits before merging"
                .to_string()
        }
        "trunk" => {
            "Git workflow: **Trunk-based**\n\
             \n\
             - Trunk-based development — small, frequent commits directly to main\n\
             - Feature flags for incomplete work\n\
             - Every commit must leave the build green\n\
             - Short-lived feature branches (< 1 day) are acceptable"
                .to_string()
        }
        _ => String::new(),
    }
}

fn render_language_rules(langs: &[String]) -> String {
    let mut rules = Vec::new();

    for lang in langs {
        match lang.as_str() {
            "rust" => rules.push(
                "**Rust:**\n\
                 - No `.unwrap()` or `.expect()` in production code — use `?` operator\n\
                 - `clippy` must be clean before marking anything done\n\
                 - Prefer `anyhow::Result` for application code, `thiserror` for library errors\n\
                 - Use `tracing` for structured logging, not `println!`"
                    .to_string(),
            ),
            "typescript" | "javascript" => rules.push(
                "**TypeScript:**\n\
                 - Strict mode always — no `any`, no `as unknown as X`\n\
                 - Prefer `const` over `let`, avoid `var`\n\
                 - Explicit return types on all exported functions\n\
                 - No `!` non-null assertions without a comment explaining why it is safe"
                    .to_string(),
            ),
            "dart" => rules.push(
                "**Dart/Flutter:**\n\
                 - Use Riverpod for state management — no `setState` in feature code\n\
                 - `flutter analyze` must be clean\n\
                 - Prefer `AsyncNotifier` for complex async state\n\
                 - No `late` variables without initialisation in the same scope"
                    .to_string(),
            ),
            "python" => rules.push(
                "**Python:**\n\
                 - Type hints required on all function signatures\n\
                 - `mypy --strict` must pass\n\
                 - Use `pathlib.Path` not `os.path` for filesystem operations\n\
                 - Prefer dataclasses or Pydantic over plain dicts for structured data"
                    .to_string(),
            ),
            "go" => rules.push(
                "**Go:**\n\
                 - Errors must be checked — never `_` an error return\n\
                 - `golangci-lint` must be clean\n\
                 - Context propagation is required for all IO-bound functions\n\
                 - Use structured logging with `slog`"
                    .to_string(),
            ),
            _ => {}
        }
    }

    if rules.is_empty() {
        "Follow idiomatic conventions for the primary language.".to_string()
    } else {
        rules.join("\n\n")
    }
}

fn render_pain_point_rules(pain_points: &[String]) -> String {
    let mut rules = Vec::new();

    for point in pain_points {
        match point.as_str() {
            "hallucinations" => rules.push(
                "- **Hallucinations:** Verify all APIs, types, and imports exist before writing them.\n  \
                 Never guess at method signatures — check the codebase first."
                    .to_string(),
            ),
            "context-loss" => rules.push(
                "- **Context loss:** Re-read the task description before each sub-step.\n  \
                 Summarise the current state of work when continuing across messages."
                    .to_string(),
            ),
            "drift" => rules.push(
                "- **Drift:** Stay strictly within the scope of the current task.\n  \
                 Do not refactor unrelated code or add unrequested features."
                    .to_string(),
            ),
            "incomplete-work" => rules.push(
                "- **Incomplete work:** Never leave stubs, TODOs, or placeholder implementations.\n  \
                 If a complete solution requires more context, ask before starting."
                    .to_string(),
            ),
            "over-engineering" => rules.push(
                "- **Over-engineering:** Implement the simplest solution that solves the problem.\n  \
                 Avoid premature abstractions and unnecessary generality."
                    .to_string(),
            ),
            _ => {}
        }
    }

    if rules.is_empty() {
        String::new()
    } else {
        format!("Specific guards:\n\n{}", rules.join("\n"))
    }
}

// ─── Codex AGENTS.md renderer ─────────────────────────────────────────────────

fn render_codex_md(answers: &QuestionnaireAnswers) -> String {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let languages = answers.primary_languages.join(", ");
    let autonomy = match answers.autonomy_level.as_str() {
        "supervised" => "Always confirm before structural changes.",
        "balanced" => "Proceed on clear tasks; confirm before structural changes.",
        "autonomous" => "Execute fully; only pause for missing credentials or destructive actions.",
        _ => "Use reasonable judgement.",
    };

    format!(
        r#"# Codex Agent Instructions

> Generated by ClawDE — {date}
> Languages: {languages}

## Role

You are a coding assistant. Execute tasks completely. Do not stop mid-task to ask
permission for obvious steps. Do what is asked — nothing more.

## Autonomy

{autonomy}

## Code Quality

- No stubs or placeholder implementations
- Tests must pass before marking work done
- Linter must be clean
- Imports must resolve — never guess at module paths

## Language Rules

{lang_rules}

## Git

{git}
"#,
        lang_rules = render_language_rules(&answers.primary_languages),
        git = match answers.git_workflow.as_str() {
            "pr-based" => "All changes via pull requests.",
            "trunk" => "Trunk-based: small frequent commits, feature flags for incomplete work.",
            _ => "Follow existing git conventions.",
        }
    )
}

// ─── Cursor rules renderer ────────────────────────────────────────────────────

fn render_cursor_rules(answers: &QuestionnaireAnswers) -> String {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let languages = answers.primary_languages.join(", ");

    format!(
        r#"# Cursor Rules
# Generated by ClawDE — {date}
# Languages: {languages}

You are an expert software engineer. Follow these rules for all code edits.

## Core Rules
- Do exactly what is asked. No unsolicited changes.
- Never stub implementations — all functions must be complete.
- Verify imports before writing them.
- No hallucinated APIs — check the codebase if unsure.

## Autonomy: {autonomy}

## Style: {style}

## Language-specific
{lang_rules}

## Git: {git}
"#,
        autonomy = answers.autonomy_level,
        style = answers.style_rules,
        lang_rules = answers
            .primary_languages
            .iter()
            .map(|l| format!("- {l}: follow idiomatic conventions and linting rules"))
            .collect::<Vec<_>>()
            .join("\n"),
        git = match answers.git_workflow.as_str() {
            "pr-based" => "PR-based workflow. All changes via pull requests.",
            "trunk" => "Trunk-based. Small commits, feature flags for incomplete work.",
            _ => "Follow existing git conventions.",
        }
    )
}

// ─── File I/O ──────────────────────────────────────────────────────────────────

/// Write `content` to `path`, creating parent directories as needed.
/// If the file exists it is backed up first.
async fn write_with_backup(path: PathBuf, content: String) -> Result<GeneratedConfig> {
    // Create parent directories.
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    let (backed_up, backup_path) = if path.exists() {
        let ts = Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let backup = path.with_extension(format!("backup.{ts}"));
        tokio::fs::copy(&path, &backup)
            .await
            .with_context(|| format!("failed to backup {}", path.display()))?;
        info!(path = %path.display(), backup = %backup.display(), "backed up existing file");
        (true, Some(backup))
    } else {
        (false, None)
    };

    tokio::fs::write(&path, &content)
        .await
        .with_context(|| format!("failed to write {}", path.display()))?;

    debug!(path = %path.display(), bytes = content.len(), "wrote generated config");

    Ok(GeneratedConfig {
        path,
        content,
        backed_up,
        backup_path,
    })
}
