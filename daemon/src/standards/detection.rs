/// Language detection — scan repo by file extension counts.
///
/// V02.T29: On session.create, scan repo by file extension counts to detect
/// primary language. Returns Unknown if the repo doesn't clearly favor one language.
use super::Language;
use std::collections::HashMap;
use std::path::Path;

/// Detect the primary language of a project by counting source file extensions.
/// Scans up to 5 directory levels deep, skips hidden dirs and vendor dirs.
pub fn detect_language(repo_path: &Path) -> Language {
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    count_extensions(repo_path, &mut counts, 0, 5);

    // Dart files → Flutter (if pubspec.yaml exists it's a Flutter/Dart project)
    let has_pubspec = repo_path.join("pubspec.yaml").exists();
    let dart_count = *counts.get("dart").unwrap_or(&0);
    let rs_count = *counts.get("rs").unwrap_or(&0);
    let ts_count = counts.get("ts").unwrap_or(&0) + counts.get("tsx").unwrap_or(&0);
    let py_count = *counts.get("py").unwrap_or(&0);
    let go_count = *counts.get("go").unwrap_or(&0);

    if has_pubspec || dart_count > 0 {
        // Dart/Flutter project
        if dart_count > 0 || has_pubspec {
            return Language::Flutter;
        }
    }

    // Find dominant language
    let totals = [
        (rs_count, Language::Rust),
        (ts_count, Language::TypeScript),
        (dart_count, Language::Flutter),
        (py_count, Language::Python),
        (go_count, Language::Go),
    ];

    let max = totals
        .iter()
        .filter(|(c, _)| *c > 0)
        .max_by_key(|(c, _)| *c);

    match max {
        Some((_, lang)) => lang.clone(),
        None => Language::Unknown,
    }
}

fn count_extensions(
    dir: &Path,
    counts: &mut HashMap<&'static str, usize>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // Skip hidden, vendor, and build dirs
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "vendor"
                || name == "dist"
                || name == "build"
            {
                continue;
            }
        }
        if path.is_dir() {
            count_extensions(&path, counts, depth + 1, max_depth);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let key: Option<&'static str> = match ext {
                "rs" => Some("rs"),
                "dart" => Some("dart"),
                "ts" => Some("ts"),
                "tsx" => Some("tsx"),
                "py" => Some("py"),
                "go" => Some("go"),
                _ => None,
            };
            if let Some(k) = key {
                *counts.entry(k).or_insert(0) += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust() {
        let tmp = TempDir::new().unwrap();
        for i in 0..5 {
            std::fs::write(tmp.path().join(format!("foo{i}.rs")), b"").unwrap();
        }
        assert_eq!(detect_language(tmp.path()), Language::Rust);
    }

    #[test]
    fn test_detect_flutter_via_pubspec() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pubspec.yaml"), b"name: myapp\n").unwrap();
        assert_eq!(detect_language(tmp.path()), Language::Flutter);
    }

    #[test]
    fn test_detect_unknown_empty() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(detect_language(tmp.path()), Language::Unknown);
    }
}
