use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

/// Global set of project roots that already have an AfsWatcher running.
/// Prevents duplicate watcher threads when `register_project` is called
/// multiple times for the same path.
static WATCHED_PROJECTS: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();

fn watched_projects() -> &'static Mutex<HashSet<String>> {
    WATCHED_PROJECTS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// AFS init: scaffold `.claude/` directory tree for a project.
pub async fn init(params: Value, _ctx: &AppContext) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing path"))?;

    // Reject relative paths and directory traversal components.
    let path_obj = std::path::Path::new(path);
    if !path_obj.is_absolute() {
        anyhow::bail!("path must be absolute");
    }
    for component in path_obj.components() {
        if matches!(component, std::path::Component::ParentDir) {
            anyhow::bail!("path must not contain '..' components");
        }
    }

    let claude_dir = std::path::Path::new(path).join(".claude");
    let mut created: Vec<String> = Vec::new();

    for dir in &[
        ".claude",
        ".claude/rules",
        ".claude/agents",
        ".claude/skills",
        ".claude/memory",
        ".claude/tasks",
        ".claude/planning",
        ".claude/qa",
        ".claude/docs",
        ".claude/archive/inbox",
        ".claude/inbox",
        ".claude/temp",
    ] {
        let full = std::path::Path::new(path).join(dir);
        if !full.exists() {
            fs::create_dir_all(&full).await?;
            created.push(dir.to_string());
        }
    }

    let claude_md = claude_dir.join("CLAUDE.md");
    if !claude_md.exists() {
        fs::write(&claude_md, CLAUDE_MD_TEMPLATE).await?;
        created.push(".claude/CLAUDE.md".to_string());
    }

    let active_md = claude_dir.join("tasks/active.md");
    if !active_md.exists() {
        fs::write(&active_md, ACTIVE_MD_TEMPLATE).await?;
        created.push(".claude/tasks/active.md".to_string());
    }

    let settings = claude_dir.join("settings.json");
    if !settings.exists() {
        fs::write(&settings, SETTINGS_JSON_TEMPLATE).await?;
        created.push(".claude/settings.json".to_string());
    }

    // Ensure .claude/ is in .gitignore
    let gitignore = std::path::Path::new(path).join(".gitignore");
    if gitignore.exists() {
        let content = fs::read_to_string(&gitignore).await.unwrap_or_default();
        if !content.contains(".claude/") {
            let updated = format!("{}\n# AI agent directories\n.claude/\n", content.trim_end());
            fs::write(&gitignore, updated).await?;
        }
    } else {
        fs::write(&gitignore, ".claude/\n").await?;
        created.push(".gitignore".to_string());
    }

    Ok(json!({ "path": path, "created": created, "ok": true }))
}

pub async fn status(params: Value, ctx: &AppContext) -> Result<Value> {
    let repo_path = params
        .get("repo_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let claude_dir = std::path::Path::new(repo_path).join(".claude");

    let has_active_md = claude_dir.join("tasks/active.md").exists();
    let has_queue_json = claude_dir.join("tasks/queue.json").exists();

    let task_counts = ctx
        .task_storage
        .summary(if repo_path.is_empty() {
            None
        } else {
            Some(repo_path)
        })
        .await?;

    Ok(json!({
        "repo_path": repo_path,
        "has_active_md": has_active_md,
        "has_queue_json": has_queue_json,
        "tasks": task_counts
    }))
}

pub async fn sync_instructions(params: Value, _ctx: &AppContext) -> Result<Value> {
    let repo_path = params
        .get("repo_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !repo_path.is_empty() {
        let rp = std::path::Path::new(repo_path);
        if !rp.is_absolute() {
            anyhow::bail!("repo_path must be absolute");
        }
        for component in rp.components() {
            if matches!(component, std::path::Component::ParentDir) {
                anyhow::bail!("repo_path must not contain '..' components");
            }
        }
    }
    let claude_md = std::path::Path::new(repo_path).join(".claude/CLAUDE.md");

    if !claude_md.exists() {
        return Ok(json!({ "ok": false, "error": "No .claude/CLAUDE.md found" }));
    }

    // Guard against reading extremely large files (e.g. if .claude/CLAUDE.md is
    // accidentally replaced with a binary or a multi-MB file).
    const MAX_CLAUDE_MD_BYTES: u64 = 512 * 1024; // 512 KB
    let meta = fs::metadata(&claude_md).await?;
    if meta.len() > MAX_CLAUDE_MD_BYTES {
        anyhow::bail!(
            "CLAUDE.md is too large ({} bytes, max {} bytes) — refusing to read",
            meta.len(),
            MAX_CLAUDE_MD_BYTES
        );
    }

    let content = fs::read_to_string(&claude_md).await?;

    let agents_md = std::path::Path::new(repo_path).join(".codex/AGENTS.md");
    if let Some(parent) = agents_md.parent() {
        fs::create_dir_all(parent).await.ok();
    }
    fs::write(&agents_md, &content).await.ok();

    Ok(json!({ "ok": true, "synced": ["codex"] }))
}

pub async fn register_project(params: Value, ctx: &AppContext) -> Result<Value> {
    let repo_path = params
        .get("repo_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing repo_path"))?;

    // Guard against duplicate watcher threads for the same project path.
    {
        let mut watched = watched_projects().lock().await;
        if watched.contains(repo_path) {
            return Ok(json!({ "ok": true, "repo_path": repo_path, "already_watching": true }));
        }
        watched.insert(repo_path.to_string());
    }

    let watcher =
        crate::tasks::watcher::AfsWatcher::new(ctx.task_storage.clone(), ctx.broadcaster.clone());
    Arc::new(watcher).watch_project(std::path::PathBuf::from(repo_path))?;

    Ok(json!({ "ok": true, "repo_path": repo_path }))
}

pub const CLAUDE_MD_TEMPLATE: &str = r#"# Project Instructions

> Read the GCI at `~/.claude/CLAUDE.md` for global protocols.

## Project Overview

[Describe this project here]

## Hard Rules

[Add project-specific rules here]
"#;

pub const ACTIVE_MD_TEMPLATE: &str = r#"# Active Tasks

> This file covers active work. Read this first at session start.

## Status Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Done |
| 🔲 | Planned |
| 🚧 | In Progress |
| 🟡 | In QA |
| 🔍 | In CR |
| ❌ | Blocked |
| 🚫 | Deferred |
| ⚠️ | Interrupted |

## Tasks

| # | Sev | Task | File | Status |
|---|-----|------|------|--------|
"#;

pub const SETTINGS_JSON_TEMPLATE: &str = r#"{
  "permissions": {
    "allow": [],
    "deny": []
  },
  "hooks": {}
}
"#;
