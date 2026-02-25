/// Stub gate — V02.T06-T08.
///
/// When a task is marked done with `modified_files`, this module greps each
/// file for stub patterns (TODO, FIXME, placeholder, stub, unimplemented!).
/// If any matches are found the completion is blocked and the caller receives
/// a `taskCompletionBlocked` error listing the match locations.
///
/// Configuration lives in `.claude/qa/completion-checks.toml` in the project
/// root. The file is optional — if absent, built-in defaults apply.

use std::path::{Path, PathBuf};
use serde::Deserialize;

// ─── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CompletionChecksConfig {
    /// Regex patterns that indicate a stub (default: built-in list).
    #[serde(default = "default_patterns")]
    pub patterns: Vec<String>,

    /// Glob patterns for files to exclude from checking (e.g. "tests/**").
    #[serde(default)]
    pub exclude: Vec<String>,

    /// File extensions to check (default: common source extensions).
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,

    /// Whether the stub gate is active at all (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_patterns() -> Vec<String> {
    vec![
        "TODO".to_string(),
        "FIXME".to_string(),
        r"\bplaceholder\b".to_string(),
        r"\bstub\b".to_string(),
        r"unimplemented!\s*\(".to_string(),
        r"todo!\s*\(".to_string(),
    ]
}

fn default_extensions() -> Vec<String> {
    vec![
        "rs".to_string(),
        "dart".to_string(),
        "ts".to_string(),
        "tsx".to_string(),
        "js".to_string(),
        "jsx".to_string(),
        "py".to_string(),
        "go".to_string(),
    ]
}

fn default_true() -> bool {
    true
}

impl Default for CompletionChecksConfig {
    fn default() -> Self {
        Self {
            patterns: default_patterns(),
            exclude: vec![],
            extensions: default_extensions(),
            enabled: true,
        }
    }
}

impl CompletionChecksConfig {
    /// Load from `.claude/qa/completion-checks.toml` in `project_root`.
    /// Returns the default config if the file doesn't exist or fails to parse.
    pub fn load(project_root: &Path) -> Self {
        let config_path = project_root.join(".claude/qa/completion-checks.toml");
        if !config_path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    tracing::warn!(
                        path = %config_path.display(),
                        err = %e,
                        "failed to parse completion-checks.toml — using defaults"
                    );
                    Self::default()
                }
            },
            Err(e) => {
                tracing::warn!(
                    path = %config_path.display(),
                    err = %e,
                    "failed to read completion-checks.toml — using defaults"
                );
                Self::default()
            }
        }
    }
}

// ─── Match result ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StubMatch {
    pub file: PathBuf,
    pub line: usize,
    pub pattern: String,
    pub content: String,
}

impl StubMatch {
    pub fn display(&self) -> String {
        format!(
            "{}:{} [{}] {}",
            self.file.display(),
            self.line,
            self.pattern,
            self.content.trim()
        )
    }
}

// ─── Gate entry point ─────────────────────────────────────────────────────────

/// Check `modified_files` for stub patterns using `config`.
///
/// Returns a non-empty `Vec<StubMatch>` when stubs are found, empty when clean.
pub fn check(
    modified_files: &[PathBuf],
    project_root: &Path,
    config: &CompletionChecksConfig,
) -> Vec<StubMatch> {
    if !config.enabled || modified_files.is_empty() {
        return vec![];
    }

    // Build combined regex from patterns.
    let combined = config.patterns.join("|");
    let re = match regex::Regex::new(&combined) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(err = %e, "invalid stub pattern regex — skipping stub gate");
            return vec![];
        }
    };

    let mut matches = Vec::new();

    for file_path in modified_files {
        // Skip files outside the project root (absolute paths from other repos).
        if !file_path.starts_with(project_root) {
            continue;
        }

        // Extension filter.
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            if !config.extensions.iter().any(|e| e == ext) {
                continue;
            }
        } else {
            continue; // No extension — skip
        }

        // Exclude glob filter.
        if is_excluded(file_path, project_root, &config.exclude) {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(file_path) {
            for (idx, line) in content.lines().enumerate() {
                if let Some(m) = re.find(line) {
                    matches.push(StubMatch {
                        file: file_path.clone(),
                        line: idx + 1,
                        pattern: m.as_str().to_string(),
                        content: line.to_string(),
                    });
                }
            }
        }
    }

    matches
}

/// Format a list of stub matches into a human-readable error message.
pub fn format_error(matches: &[StubMatch]) -> String {
    let mut msg = format!(
        "taskCompletionBlocked: {} stub pattern{} found in modified files:\n",
        matches.len(),
        if matches.len() == 1 { "" } else { "s" }
    );
    for m in matches.iter().take(20) {
        msg.push_str(&format!("  {}\n", m.display()));
    }
    if matches.len() > 20 {
        msg.push_str(&format!("  … and {} more\n", matches.len() - 20));
    }
    msg.push_str("\nResolve all stubs before marking the task done.");
    msg
}

// ─── Exclude helper ───────────────────────────────────────────────────────────

fn is_excluded(file: &Path, project_root: &Path, excludes: &[String]) -> bool {
    if excludes.is_empty() {
        return false;
    }
    let rel = match file.strip_prefix(project_root) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let rel_str = rel.to_string_lossy();
    for pattern in excludes {
        if glob_match(pattern, &rel_str) {
            return true;
        }
    }
    false
}

/// Simple glob matching: only supports `*` (any segment) and `**` (any path).
fn glob_match(pattern: &str, path: &str) -> bool {
    // Delegate to regex via simple conversion: ** → .*, * → [^/]*
    let regex_str = pattern
        .replace("**", "\x00") // placeholder
        .replace('*', "[^/]*")
        .replace('\x00', ".*");
    regex::Regex::new(&format!("^{regex_str}$"))
        .map(|re| re.is_match(path))
        .unwrap_or(false)
}
