// instructions/snapshot.rs — Instruction snapshot tests (Sprint ZZ PT.T03)
//
// Golden file comparison for compiled instruction output.
// `clawd instructions snapshot --path <dir>` writes a golden file.
// `clawd instructions snapshot --check` compares against the golden.

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Write a snapshot of the effective compiled instructions to a golden file.
///
/// Output path: `{base_dir}/.instruction-snapshot.md`
pub async fn write_snapshot(
    compiled_content: &str,
    snapshot_path: &Path,
) -> Result<()> {
    let header = format!(
        "<!-- instruction-snapshot — do not edit manually\n\
         hash: {}\n\
         generated: {}\n-->\n",
        content_hash(compiled_content),
        chrono::Utc::now().to_rfc3339(),
    );

    let full = format!("{}{}", header, compiled_content);
    tokio::fs::write(snapshot_path, &full).await?;
    Ok(())
}

/// Check current compiled instructions against a golden file.
///
/// Returns (matches, diff_summary).
pub async fn check_snapshot(
    compiled_content: &str,
    snapshot_path: &Path,
) -> Result<(bool, String)> {
    if !snapshot_path.exists() {
        return Ok((
            false,
            format!(
                "No snapshot file found at {}. Run `clawd instructions snapshot` first.",
                snapshot_path.display()
            ),
        ));
    }

    let golden = tokio::fs::read_to_string(snapshot_path).await?;

    // Extract hash from golden header
    let golden_hash = extract_hash(&golden);
    let current_hash = content_hash(compiled_content);

    if golden_hash.as_deref() == Some(current_hash.as_str()) {
        return Ok((true, String::new()));
    }

    // Produce a line-diff summary
    let golden_body = strip_header(&golden);
    let diff = compute_line_diff(golden_body, compiled_content);

    Ok((false, diff))
}

fn content_hash(content: &str) -> String {
    let hash = Sha256::digest(content.as_bytes());
    hex::encode(&hash[..16])
}

fn extract_hash(snapshot_content: &str) -> Option<String> {
    for line in snapshot_content.lines() {
        if let Some(rest) = line.strip_prefix("hash: ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn strip_header(snapshot_content: &str) -> &str {
    if let Some(end) = snapshot_content.find("-->") {
        let after = end + 3;
        if after < snapshot_content.len() {
            return snapshot_content[after..].trim_start_matches('\n');
        }
    }
    snapshot_content
}

/// Simple line diff — counts added/removed lines.
fn compute_line_diff(golden: &str, current: &str) -> String {
    let golden_lines: Vec<&str> = golden.lines().collect();
    let current_lines: Vec<&str> = current.lines().collect();

    let added = current_lines
        .iter()
        .filter(|l| !golden_lines.contains(l))
        .count();
    let removed = golden_lines
        .iter()
        .filter(|l| !current_lines.contains(l))
        .count();

    format!(
        "Instructions have drifted: ~{removed} lines removed, ~{added} lines added. \
         Re-run `clawd instructions snapshot` to update the golden file."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_hash_roundtrip() {
        let content = "# Rule: use pnpm\nAlways use pnpm, never npm.";
        let hash1 = content_hash(content);
        let hash2 = content_hash(content);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_extract_hash() {
        let snapshot = "<!-- instruction-snapshot\nhash: abc123\ngenerated: 2026-01-01\n-->\n# Content";
        assert_eq!(extract_hash(snapshot), Some("abc123".to_string()));
    }
}
