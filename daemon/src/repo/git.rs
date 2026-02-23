use anyhow::{Context, Result};
use chrono::Utc;
use git2::{Repository, StatusOptions};
use serde::Serialize;

// ─── Types matching @clawde/proto ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub repo_path: String,
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub files: Vec<FileStatusEntry>,
    pub has_conflicts: bool,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatusEntry {
    pub path: String,
    pub status: FileStatusKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatusKind {
    Clean,
    Modified,
    Staged,
    Deleted,
    Untracked,
    Conflict,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub hunks: Vec<DiffHunk>,
    pub is_binary: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    #[serde(rename = "type")]
    pub kind: DiffLineKind,
    pub content: String,
    pub old_line_no: Option<u32>,
    pub new_line_no: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

// ─── Status reading ──────────────────────────────────────────────────────────

pub fn read_status(repo: &Repository) -> Result<RepoStatus> {
    let repo_path = repo
        .workdir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let branch = current_branch(repo).unwrap_or_else(|_| "HEAD".to_string());

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .recurse_untracked_dirs(true)
        // Show submodule changes (indexed submodule SHA vs actual HEAD).
        // git2 includes submodule entries by default; this ensures they are
        // not silently skipped even on older git2 versions.
        .exclude_submodules(false);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut files = Vec::new();
    let mut has_conflicts = false;

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let s = entry.status();

        if s.is_conflicted() {
            has_conflicts = true;
            files.push(FileStatusEntry {
                path,
                status: FileStatusKind::Conflict,
            });
            continue;
        }
        if s.is_wt_new() {
            files.push(FileStatusEntry {
                path,
                status: FileStatusKind::Untracked,
            });
        } else if s.is_index_new()
            || s.is_index_modified()
            || s.is_index_deleted()
            || s.is_index_renamed()
        {
            files.push(FileStatusEntry {
                path,
                status: FileStatusKind::Staged,
            });
        } else if s.is_wt_modified() || s.is_wt_renamed() {
            files.push(FileStatusEntry {
                path,
                status: FileStatusKind::Modified,
            });
        } else if s.is_wt_deleted() || s.is_index_deleted() {
            files.push(FileStatusEntry {
                path,
                status: FileStatusKind::Deleted,
            });
        }
    }

    let (ahead, behind) = ahead_behind(repo).unwrap_or((0, 0));

    Ok(RepoStatus {
        repo_path,
        branch,
        ahead,
        behind,
        files,
        has_conflicts,
        last_updated: Utc::now().to_rfc3339(),
    })
}

fn current_branch(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    if head.is_branch() {
        Ok(head.shorthand().unwrap_or("HEAD").to_string())
    } else {
        // Detached HEAD — show short SHA
        let oid = head.peel_to_commit()?.id();
        Ok(format!("{:.7}", oid))
    }
}

fn ahead_behind(repo: &Repository) -> Result<(usize, usize)> {
    let head = repo.head()?;
    if !head.is_branch() {
        return Ok((0, 0));
    }
    let branch_name = head.shorthand().unwrap_or("HEAD");
    let local = head.peel_to_commit()?.id();
    // Look for upstream tracking ref
    let upstream_ref = format!("refs/remotes/origin/{}", branch_name);
    let upstream = match repo.find_reference(&upstream_ref) {
        Ok(r) => r.peel_to_commit()?.id(),
        Err(_) => return Ok((0, 0)),
    };
    let (ahead, behind) = repo.graph_ahead_behind(local, upstream)?;
    Ok((ahead, behind))
}

// ─── Diff reading ────────────────────────────────────────────────────────────

pub fn read_diff(repo: &Repository) -> Result<Vec<FileDiff>> {
    let head_tree = match repo.head() {
        Ok(h) => Some(h.peel_to_tree()?),
        Err(_) => None,
    };
    let diff = repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), None)?;
    parse_diff(diff)
}

pub fn read_file_diff(repo: &Repository, path: &str, staged: bool) -> Result<FileDiff> {
    // Submodule entries appear as directories in the working tree.
    // git2 can't produce a normal file diff for them — return a summary instead.
    if let Some(workdir) = repo.workdir() {
        if workdir.join(path).is_dir() {
            // Try to read the submodule's current HEAD commit for context.
            let submodule_note = repo
                .find_submodule(path)
                .ok()
                .and_then(|sm| sm.head_id())
                .map(|id| format!("HEAD: {:.7}", id))
                .unwrap_or_else(|| "submodule".to_string());

            return Ok(FileDiff {
                path: path.to_string(),
                old_path: None,
                hunks: vec![DiffHunk {
                    header: "@@ submodule @@".to_string(),
                    old_start: 0,
                    old_lines: 0,
                    new_start: 0,
                    new_lines: 1,
                    lines: vec![DiffLine {
                        kind: DiffLineKind::Context,
                        content: format!("Submodule {} ({})", path, submodule_note),
                        old_line_no: None,
                        new_line_no: Some(1),
                    }],
                }],
                is_binary: false,
            });
        }
    }

    let head_tree = match repo.head() {
        Ok(h) => Some(h.peel_to_tree()?),
        Err(_) => None,
    };

    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);

    let diff = if staged {
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };

    let diffs = parse_diff(diff)?;
    diffs
        .into_iter()
        .next()
        .context(format!("no diff found for path: {}", path))
}

fn parse_diff(diff: git2::Diff) -> Result<Vec<FileDiff>> {
    let mut result: Vec<FileDiff> = Vec::new();

    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
        let new_file = delta
            .new_file()
            .path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let old_file = delta
            .old_file()
            .path()
            .map(|p| p.to_string_lossy().into_owned());
        let is_binary = delta.new_file().is_binary();

        // Find or create the FileDiff entry
        if result.last().map(|f: &FileDiff| f.path.as_str()) != Some(&new_file) {
            result.push(FileDiff {
                path: new_file.clone(),
                old_path: old_file.filter(|p| p != &new_file),
                hunks: Vec::new(),
                is_binary,
            });
        }

        let file = match result.last_mut() {
            Some(f) => f,
            None => return false, // should not happen — entry was just pushed
        };

        if let Some(hunk) = hunk {
            let header = std::str::from_utf8(hunk.header())
                .unwrap_or("")
                .trim()
                .to_string();
            if file.hunks.last().map(|h: &DiffHunk| h.header.as_str()) != Some(&header) {
                file.hunks.push(DiffHunk {
                    header: header.clone(),
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                    lines: Vec::new(),
                });
            }
        }

        if let Some(current_hunk) = file.hunks.last_mut() {
            let content = std::str::from_utf8(line.content())
                .unwrap_or("")
                .trim_end_matches('\n')
                .to_string();

            let kind = match line.origin() {
                '+' => DiffLineKind::Added,
                '-' => DiffLineKind::Removed,
                _ => DiffLineKind::Context,
            };
            current_hunk.lines.push(DiffLine {
                kind,
                content,
                old_line_no: line.old_lineno(),
                new_line_no: line.new_lineno(),
            });
        }

        true
    })?;

    Ok(result)
}
