// SPDX-License-Identifier: MIT
/// VS Code extension detection — Sprint S, LS.T05 / LS.T09.
///
/// Reads `.vscode/extensions.json` from a workspace and detects which VS Code
/// extensions are recommended or installed for that project.  Also provides
/// the path to the user's VS Code global extension installation directory so
/// callers can check whether an extension is actually present on disk.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

// ─── Public types ─────────────────────────────────────────────────────────────

/// Metadata about a single VS Code extension found in the workspace or the
/// global extension install directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    /// Publisher + name in the standard `publisher.name` format.
    pub id: String,
    /// Human-readable display name (derived from the id when not available).
    pub name: String,
    /// Version string (empty if unknown / extension not locally installed).
    pub version: String,
    /// `true` if the extension directory is present in the VS Code extensions
    /// folder on disk (i.e. it is actually installed, not just recommended).
    pub enabled: bool,
}

// ─── Internal types ───────────────────────────────────────────────────────────

/// Shape of `.vscode/extensions.json`.
#[derive(Deserialize)]
struct VscodeExtensionsFile {
    recommendations: Option<Vec<String>>,
    #[serde(rename = "unwantedRecommendations")]
    unwanted_recommendations: Option<Vec<String>>,
}

/// Shape of an extension's `package.json` (only fields we care about).
#[derive(Deserialize)]
struct ExtensionPackageJson {
    name: Option<String>,
    version: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

// ─── VS Code global extension directory ──────────────────────────────────────

/// Return the platform-specific path to the VS Code user extension directory.
///
/// VS Code installs extensions under:
/// - macOS: `~/.vscode/extensions`
/// - Linux: `~/.vscode/extensions`
/// - Windows: `%USERPROFILE%\.vscode\extensions`
///
/// Returns `None` if the home directory cannot be determined.
pub fn vscode_extensions_dir() -> Option<PathBuf> {
    let home = dirs_home()?;
    let path = home.join(".vscode").join("extensions");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Portable home directory resolution without an external crate dependency.
fn dirs_home() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

// ─── Detection ────────────────────────────────────────────────────────────────

/// Read `.vscode/extensions.json` from `workspace` and cross-reference against
/// the VS Code extension install directory to build a list of `ExtensionInfo`.
///
/// Extensions listed in `recommendations` are included regardless of whether
/// they are installed.  Extensions listed in `unwantedRecommendations` are
/// excluded entirely.
///
/// If `.vscode/extensions.json` does not exist, returns an empty list.
pub fn detect_vscode_extensions(workspace: &Path) -> Vec<ExtensionInfo> {
    let extensions_json = workspace.join(".vscode").join("extensions.json");
    if !extensions_json.exists() {
        debug!("no .vscode/extensions.json found");
        return Vec::new();
    }

    let content = match std::fs::read_to_string(&extensions_json) {
        Ok(c) => c,
        Err(e) => {
            debug!(err = %e, "could not read .vscode/extensions.json");
            return Vec::new();
        }
    };

    // VS Code allows comments in extensions.json (it uses JSON-with-comments),
    // so we strip single-line `//` comments before parsing.
    let cleaned = strip_line_comments(&content);

    let parsed: VscodeExtensionsFile = match serde_json::from_str(&cleaned) {
        Ok(p) => p,
        Err(e) => {
            debug!(err = %e, "could not parse .vscode/extensions.json");
            return Vec::new();
        }
    };

    let unwanted: std::collections::HashSet<String> = parsed
        .unwanted_recommendations
        .unwrap_or_default()
        .into_iter()
        .map(|id| id.to_lowercase())
        .collect();

    let recommendations = parsed.recommendations.unwrap_or_default();
    let ext_dir = vscode_extensions_dir();

    recommendations
        .into_iter()
        .filter(|id| !unwanted.contains(&id.to_lowercase()))
        .map(|id| {
            let (name, version, enabled) =
                probe_installed_extension(&id, ext_dir.as_deref());
            ExtensionInfo {
                name: name.unwrap_or_else(|| extension_display_name(&id)),
                version: version.unwrap_or_default(),
                enabled,
                id,
            }
        })
        .collect()
}

/// Try to find the extension on disk in the VS Code extensions directory.
///
/// VS Code installs extensions as `{publisher}.{name}-{version}/` directories.
/// Returns `(display_name, version, is_installed)`.
fn probe_installed_extension(
    id: &str,
    ext_dir: Option<&Path>,
) -> (Option<String>, Option<String>, bool) {
    let ext_dir = match ext_dir {
        Some(d) => d,
        None => return (None, None, false),
    };

    // The directory name starts with the id (case-insensitive) followed by a dash and version
    let prefix = id.to_lowercase();
    let read_dir = match std::fs::read_dir(ext_dir) {
        Ok(d) => d,
        Err(_) => return (None, None, false),
    };

    for entry in read_dir.flatten() {
        let dir_name = entry.file_name().to_string_lossy().to_lowercase();
        if dir_name.starts_with(&prefix) {
            // Try to read the extension's package.json for name + version
            let pkg_json = entry.path().join("package.json");
            if let Ok(content) = std::fs::read_to_string(&pkg_json) {
                if let Ok(pkg) = serde_json::from_str::<ExtensionPackageJson>(&content) {
                    return (
                        pkg.display_name.or(pkg.name),
                        pkg.version,
                        true,
                    );
                }
            }
            return (None, None, true);
        }
    }

    (None, None, false)
}

/// Derive a human-readable display name from an extension id.
///
/// For example, `"rust-lang.rust-analyzer"` → `"rust-analyzer"`.
fn extension_display_name(id: &str) -> String {
    id.splitn(2, '.').nth(1).unwrap_or(id).to_string()
}

/// Strip `//` single-line comments from JSON text (VS Code JSONC dialect).
///
/// This is a best-effort implementation that handles the most common cases.
/// It does not handle `/* */` block comments or `//` inside string literals.
fn strip_line_comments(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            // Find `//` that is not inside a string — very naive heuristic:
            // count unescaped `"` characters before the `//` position.
            let mut in_string = false;
            let mut prev = '\0';
            let chars: Vec<char> = line.chars().collect();
            let mut cut = chars.len();
            let mut i = 0;
            while i < chars.len() {
                let c = chars[i];
                if c == '"' && prev != '\\' {
                    in_string = !in_string;
                }
                if !in_string && c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                    cut = i;
                    break;
                }
                prev = c;
                i += 1;
            }
            chars[..cut].iter().collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
