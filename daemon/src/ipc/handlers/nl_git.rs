//! Sprint DD NL.1/NL.2/NL.3 — `git.query` — Natural Language Git History.
//!
//! Takes a natural language question about the git history and returns a
//! structured answer with a commit list and a human narrative.
//!
//! ## How it works
//!
//! 1. Parse the question for time filters ("last week", "yesterday") and
//!    path filters ("in auth", "touching payments").
//! 2. Run `git log` with derived parameters.
//! 3. Synthesize a one-paragraph narrative from the commit list using
//!    simple template formatting (no live AI call in local mode).

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use std::process::Stdio;

/// `git.query` — natural language git history query.
pub async fn query(params: Value, _ctx: &AppContext) -> Result<Value> {
    let question = params
        .get("question")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("question required"))?;
    let repo_path = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let args = derive_git_log_args(question);
    let commits = run_git_log(repo_path, &args).await?;
    let narrative = synthesize_narrative(question, &commits);

    Ok(json!({
        "question": question,
        "narrative": narrative,
        "commits": commits,
        "gitArgs": args,
    }))
}

/// Derive git log arguments from a natural language question.
fn derive_git_log_args(question: &str) -> Vec<String> {
    let q = question.to_lowercase();
    let mut args = vec![
        "log".to_string(),
        "--oneline".to_string(),
        "--no-merges".to_string(),
        "--format=%H|||%s|||%an|||%ad".to_string(),
        "--date=short".to_string(),
    ];

    // Time filters.
    if q.contains("yesterday") {
        args.push("--since=yesterday".to_string());
        args.push("--until=today".to_string());
    } else if q.contains("last week") || q.contains("past week") {
        args.push("--since=7 days ago".to_string());
    } else if q.contains("last month") || q.contains("past month") {
        args.push("--since=30 days ago".to_string());
    } else if q.contains("today") {
        args.push("--since=midnight".to_string());
    } else {
        args.push("--since=7 days ago".to_string());
    }

    // Count limit.
    if q.contains("last 3") || q.contains("latest 3") {
        args.push("-3".to_string());
    } else if q.contains("last 5") || q.contains("latest 5") {
        args.push("-5".to_string());
    } else if q.contains("last 10") {
        args.push("-10".to_string());
    } else {
        args.push("-20".to_string());
    }

    // Path filter: "in auth", "in src/", "touching payments".
    for keyword in &["in ", "touching ", "for ", "related to "] {
        if let Some(pos) = q.find(keyword) {
            let after = &q[pos + keyword.len()..];
            let word: String = after
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '_')
                .to_string();
            if !word.is_empty() && word.len() > 2 {
                args.push("--".to_string());
                args.push(format!("*{}*", word));
                break;
            }
        }
    }

    args
}

/// Run `git log` and parse the output into structured commits.
async fn run_git_log(repo_path: &str, args: &[String]) -> Result<Vec<Value>> {
    let output = tokio::process::Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<Value> = stdout
        .lines()
        .filter(|l| l.contains("|||"))
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, "|||").collect();
            if parts.len() >= 4 {
                Some(json!({
                    "sha": &parts[0][..7.min(parts[0].len())],
                    "subject": parts[1],
                    "author": parts[2],
                    "date": parts[3],
                }))
            } else {
                None
            }
        })
        .collect();

    Ok(commits)
}

/// Generate a human-readable narrative from the commit list.
fn synthesize_narrative(question: &str, commits: &[Value]) -> String {
    if commits.is_empty() {
        return format!("No commits found matching: \"{}\".", question);
    }

    let count = commits.len();
    let period = if question.to_lowercase().contains("yesterday") {
        "yesterday"
    } else if question.to_lowercase().contains("last week") {
        "last week"
    } else {
        "recently"
    };

    let mut parts = vec![format!(
        "{} commit{} {}",
        count,
        if count == 1 { "" } else { "s" },
        period
    )];

    // List up to 3 commits by name.
    for (i, commit) in commits.iter().take(3).enumerate() {
        let subject = commit.get("subject").and_then(|v| v.as_str()).unwrap_or("");
        let sha = commit.get("sha").and_then(|v| v.as_str()).unwrap_or("");
        let author = commit.get("author").and_then(|v| v.as_str()).unwrap_or("");

        if i == 0 {
            parts.push(format!(": {} ({}, {})", subject, sha, author));
        } else {
            parts.push(format!("; {} ({}, {})", subject, sha, author));
        }
    }

    if count > 3 {
        parts.push(format!(" and {} more", count - 3));
    }

    parts.push(".".to_string());
    parts.join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_args_last_week() {
        let args = derive_git_log_args("what changed in auth last week?");
        assert!(args.iter().any(|a| a.contains("7 days ago")));
    }

    #[test]
    fn test_derive_args_yesterday() {
        let args = derive_git_log_args("what did I commit yesterday?");
        assert!(args.iter().any(|a| a == "--since=yesterday"));
    }

    #[test]
    fn test_narrative_empty() {
        let n = synthesize_narrative("test", &[]);
        assert!(n.contains("No commits"));
    }

    #[test]
    fn test_narrative_single() {
        let commits = vec![json!({
            "sha": "abc1234",
            "subject": "fix: auth bug",
            "author": "Alice",
            "date": "2026-02-26",
        })];
        let n = synthesize_narrative("what changed yesterday", &commits);
        assert!(n.contains("1 commit"));
        assert!(n.contains("auth bug"));
    }
}
