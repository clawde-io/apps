// tasks/ownership.rs — File ownership enforcement (Sprint ZZ FO.T02, FO.T03, DR.T03)
//
// Enforces that agents only touch files within their task's declared owned_paths.
// Generates a PreToolUse hook that queries the daemon before each Write/Edit.

use crate::storage::Storage;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Simple glob matcher: supports `*` (any chars except `/`) and `**` (any chars incl `/`).
fn glob_matches(pattern: &str, path: &str) -> bool {
    glob_matches_inner(pattern.as_bytes(), path.as_bytes())
}

fn glob_matches_inner(pat: &[u8], text: &[u8]) -> bool {
    let mut p = 0;
    let mut t = 0;
    let mut star_p: Option<usize> = None;
    let mut star_t: usize = 0;

    while t < text.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p + 1 < pat.len() && pat[p] == b'*' && pat[p + 1] == b'*' {
            // `**` — match any characters including `/`
            star_p = Some(p);
            star_t = t;
            p += 2;
            if p < pat.len() && pat[p] == b'/' { p += 1; }
        } else if p < pat.len() && pat[p] == b'*' {
            // `*` — match any characters except `/`
            star_p = Some(p);
            star_t = t;
            p += 1;
        } else if let Some(sp) = star_p {
            // backtrack to last star
            let last_star_double = sp + 1 < pat.len() && pat[sp + 1] == b'*';
            // For single `*`, don't cross `/`
            if !last_star_double && text[star_t] == b'/' {
                star_p = None;
                p = sp;
                t = star_t + 1;
                continue;
            }
            star_t += 1;
            t = star_t;
            p = sp + if last_star_double { 2 } else { 1 };
        } else {
            return false;
        }
    }

    while p + 1 < pat.len() && pat[p] == b'*' && pat[p + 1] == b'*' { p += 2; }
    while p < pat.len() && pat[p] == b'*' { p += 1; }

    p == pat.len()
}

/// Result of checking whether a path is within a task's ownership set.
#[derive(Debug, Serialize, Deserialize)]
pub struct OwnershipCheck {
    pub allowed: bool,
    pub task_id: String,
    pub path: String,
    /// Which glob pattern matched (if allowed), or empty string.
    pub matched_pattern: Option<String>,
    /// Reason for denial (if not allowed).
    pub denial_reason: Option<String>,
}

/// Checks if `path` is within any of the glob patterns in `owned_paths_json`.
pub fn check_path_ownership(task_id: &str, owned_paths_json: &str, path: &str) -> OwnershipCheck {
    // Empty ownership = no restrictions (task didn't declare scope)
    if owned_paths_json.is_empty() || owned_paths_json == "[]" || owned_paths_json == "null" {
        return OwnershipCheck {
            allowed: true,
            task_id: task_id.to_string(),
            path: path.to_string(),
            matched_pattern: None,
            denial_reason: None,
        };
    }

    let patterns: Vec<String> = serde_json::from_str(owned_paths_json).unwrap_or_default();
    if patterns.is_empty() {
        return OwnershipCheck {
            allowed: true,
            task_id: task_id.to_string(),
            path: path.to_string(),
            matched_pattern: None,
            denial_reason: None,
        };
    }

    for pattern_str in &patterns {
        if glob_matches(pattern_str, path) {
            return OwnershipCheck {
                allowed: true,
                task_id: task_id.to_string(),
                path: path.to_string(),
                matched_pattern: Some(pattern_str.clone()),
                denial_reason: None,
            };
        }
    }

    OwnershipCheck {
        allowed: false,
        task_id: task_id.to_string(),
        path: path.to_string(),
        matched_pattern: None,
        denial_reason: Some(format!(
            "Path '{path}' is outside task {task_id}'s declared owned_paths. \
             Use `clawd task expand-ownership --task {task_id} --add <glob>` to expand scope."
        )),
    }
}

/// FO.T02 — Given a task description + repo structure analysis, suggest owned_paths globs.
///
/// Called by FORGE when creating task specs.
pub fn suggest_owned_paths(task_title: &str, task_files: &[&str]) -> Vec<String> {
    let mut globs: Vec<String> = Vec::new();

    // Direct file matches
    for file in task_files {
        globs.push((*file).to_string());
    }

    // Infer directory globs from file patterns
    let dirs: std::collections::HashSet<String> = task_files
        .iter()
        .filter_map(|f| {
            let p = std::path::Path::new(f);
            p.parent()
                .map(|d| format!("{}/**", d.to_string_lossy()))
        })
        .collect();

    globs.extend(dirs);

    // Add test files if source files are present
    let has_rust = task_files.iter().any(|f| f.ends_with(".rs"));
    let has_ts = task_files
        .iter()
        .any(|f| f.ends_with(".ts") || f.ends_with(".tsx"));
    let has_dart = task_files.iter().any(|f| f.ends_with(".dart"));

    if has_rust {
        if task_title.to_lowercase().contains("test") {
            globs.push("tests/**".to_string());
        }
    }
    if has_ts {
        globs.push("**/*.test.ts".to_string());
        globs.push("**/*.test.tsx".to_string());
    }
    if has_dart {
        globs.push("test/**".to_string());
    }

    globs.dedup();
    globs
}

/// FO.T03 — Generate `.claude/settings.json` hook content for ownership checking.
///
/// The generated hook queries the daemon before each Write/Edit tool call.
pub fn generate_ownership_hook_content(daemon_port: u16) -> String {
    format!(
        r#"{{
  "hooks": {{
    "PreToolUse": [
      {{
        "matcher": "Write|Edit|NotebookEdit",
        "hooks": [
          {{
            "type": "command",
            "command": "clawd tasks check-ownership --path \"${{tool_input.file_path:-${{tool_input.path}}}}\" --port {daemon_port}"
          }}
        ]
      }}
    ]
  }}
}}"#
    )
}

/// Check for owned-path overlaps between two tasks at claim time.
///
/// Returns a list of conflicting path patterns.
pub fn check_ownership_overlap(
    claimed_paths_json: &str,
    existing_paths_json: &str,
) -> Vec<String> {
    let claimed: Vec<String> =
        serde_json::from_str(claimed_paths_json).unwrap_or_default();
    let existing: Vec<String> =
        serde_json::from_str(existing_paths_json).unwrap_or_default();

    let mut conflicts = Vec::new();

    for c in &claimed {
        for e in &existing {
            // Simple heuristic: if one pattern is a prefix of the other, they overlap
            if c == e
                || c.trim_end_matches("/**") == e.trim_end_matches("/**")
                || c.starts_with(e.trim_end_matches("/**"))
                || e.starts_with(c.trim_end_matches("/**"))
            {
                conflicts.push(format!("{c} ↔ {e}"));
            }
        }
    }
    conflicts
}

/// DR.T03 — CRUNCH budget gate: check if a task touches files outside its declared scope.
///
/// Returns a list of out-of-scope paths.
pub fn files_outside_ownership(
    owned_paths_json: &str,
    touched_paths: &[String],
) -> Vec<String> {
    let patterns: Vec<String> =
        serde_json::from_str(owned_paths_json).unwrap_or_default();

    if patterns.is_empty() {
        return Vec::new(); // No restrictions
    }

    touched_paths
        .iter()
        .filter(|path| {
            !patterns.iter().any(|pat_str| glob_matches(pat_str, path.as_str()))
        })
        .cloned()
        .collect()
}

/// Expand a task's owned_paths JSON array with new patterns.
pub fn expand_owned_paths(existing_json: &str, new_patterns: &[String]) -> String {
    let mut existing: Vec<String> =
        serde_json::from_str(existing_json).unwrap_or_default();
    for p in new_patterns {
        if !existing.contains(p) {
            existing.push(p.clone());
        }
    }
    serde_json::to_string(&existing).unwrap_or_else(|_| "[]".to_string())
}

pub struct OwnershipStorage<'a> {
    pub storage: &'a Storage,
}

impl<'a> OwnershipStorage<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// Set owned_paths for a task.
    pub async fn set_ownership(&self, task_id: &str, paths: &[String]) -> Result<()> {
        let json = serde_json::to_string(paths)?;
        sqlx::query("UPDATE agent_tasks SET owned_paths_json = ? WHERE id = ?")
            .bind(&json)
            .bind(task_id)
            .execute(self.storage.pool())
            .await?;
        Ok(())
    }

    /// Expand owned_paths for a task (add new patterns).
    pub async fn expand_ownership(&self, task_id: &str, new_paths: &[String]) -> Result<String> {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT owned_paths_json FROM agent_tasks WHERE id = ?")
                .bind(task_id)
                .fetch_optional(self.storage.pool())
                .await?
                .flatten();

        let existing_json = existing.unwrap_or_else(|| "[]".to_string());
        let expanded = expand_owned_paths(&existing_json, new_paths);

        sqlx::query("UPDATE agent_tasks SET owned_paths_json = ? WHERE id = ?")
            .bind(&expanded)
            .bind(task_id)
            .execute(self.storage.pool())
            .await?;

        Ok(expanded)
    }

    /// Check if two tasks have overlapping owned paths at claim time.
    pub async fn check_claim_overlap(
        &self,
        task_id: &str,
        new_paths_json: &str,
    ) -> Result<Vec<String>> {
        // Get all currently-claimed tasks (excluding the one being claimed)
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, owned_paths_json FROM agent_tasks \
             WHERE status IN ('claimed', 'active') AND id != ? AND owned_paths_json IS NOT NULL",
        )
        .bind(task_id)
        .fetch_all(self.storage.pool())
        .await?;

        let mut all_conflicts = Vec::new();
        for (other_id, other_paths) in &rows {
            let conflicts = check_ownership_overlap(new_paths_json, other_paths);
            if !conflicts.is_empty() {
                all_conflicts.push(format!("Task {other_id}: {}", conflicts.join(", ")));
            }
        }
        Ok(all_conflicts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_ownership_allows_matching_glob() {
        let result = check_path_ownership(
            "T-001",
            r#"["src/payments/**"]"#,
            "src/payments/handler.rs",
        );
        assert!(result.allowed);
    }

    #[test]
    fn test_check_ownership_denies_outside_glob() {
        let result = check_path_ownership(
            "T-001",
            r#"["src/payments/**"]"#,
            "src/session/manager.rs",
        );
        assert!(!result.allowed);
        assert!(result.denial_reason.is_some());
    }

    #[test]
    fn test_empty_ownership_allows_all() {
        let result = check_path_ownership("T-001", "[]", "src/anything.rs");
        assert!(result.allowed);
    }

    #[test]
    fn test_overlap_detection() {
        let conflicts =
            check_ownership_overlap(r#"["src/payments/**"]"#, r#"["src/payments/handler.rs"]"#);
        assert!(!conflicts.is_empty());
    }

    #[test]
    fn test_no_overlap() {
        let conflicts =
            check_ownership_overlap(r#"["src/session/**"]"#, r#"["src/payments/**"]"#);
        assert!(conflicts.is_empty());
    }
}
