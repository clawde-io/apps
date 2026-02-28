/// Data types for the Repo Intelligence subsystem (Sprint F).
use serde::{Deserialize, Serialize};

/// A detected programming language.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PrimaryLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Dart,
    Python,
    Go,
    Ruby,
    Swift,
    Kotlin,
    Java,
    Unknown,
}

impl PrimaryLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrimaryLanguage::Rust => "rust",
            PrimaryLanguage::TypeScript => "typescript",
            PrimaryLanguage::JavaScript => "javascript",
            PrimaryLanguage::Dart => "dart",
            PrimaryLanguage::Python => "python",
            PrimaryLanguage::Go => "go",
            PrimaryLanguage::Ruby => "ruby",
            PrimaryLanguage::Swift => "swift",
            PrimaryLanguage::Kotlin => "kotlin",
            PrimaryLanguage::Java => "java",
            PrimaryLanguage::Unknown => "unknown",
        }
    }

    pub fn from_tag(s: &str) -> Self {
        match s {
            "rust" => PrimaryLanguage::Rust,
            "typescript" => PrimaryLanguage::TypeScript,
            "javascript" => PrimaryLanguage::JavaScript,
            "dart" => PrimaryLanguage::Dart,
            "python" => PrimaryLanguage::Python,
            "go" => PrimaryLanguage::Go,
            "ruby" => PrimaryLanguage::Ruby,
            "swift" => PrimaryLanguage::Swift,
            "kotlin" => PrimaryLanguage::Kotlin,
            "java" => PrimaryLanguage::Java,
            _ => PrimaryLanguage::Unknown,
        }
    }
}

/// A detected web/backend framework or tooling environment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Framework {
    NextJs,
    Vite,
    Tailwind,
    Docker,
    GithubActions,
    Cursor,
    ClaudeCode,
    #[serde(untagged)]
    Other(String),
}

impl Framework {
    pub fn as_str(&self) -> &str {
        match self {
            Framework::NextJs => "nextjs",
            Framework::Vite => "vite",
            Framework::Tailwind => "tailwind",
            Framework::Docker => "docker",
            Framework::GithubActions => "github_actions",
            Framework::Cursor => "cursor",
            Framework::ClaudeCode => "claude_code",
            Framework::Other(s) => s.as_str(),
        }
    }
}

/// A detected build orchestration tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BuildTool {
    Make,
    Just,
    Turbo,
    Nx,
    Melos,
    Taskfile,
    #[serde(untagged)]
    Other(String),
}

impl BuildTool {
    pub fn as_str(&self) -> &str {
        match self {
            BuildTool::Make => "make",
            BuildTool::Just => "just",
            BuildTool::Turbo => "turbo",
            BuildTool::Nx => "nx",
            BuildTool::Melos => "melos",
            BuildTool::Taskfile => "taskfile",
            BuildTool::Other(s) => s.as_str(),
        }
    }
}

/// Inferred source-code style conventions for a repository.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeConventions {
    /// Detected function/variable naming style: "snake_case" | "camelCase" | "PascalCase" | "mixed"
    pub naming_style: Option<String>,
    /// Detected indentation: "tabs" | "2-space" | "4-space"
    pub indentation: Option<String>,
    /// Approximate max line length observed in the sample
    pub max_line_length: Option<usize>,
}

/// Full profile produced by `repo.scan`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoProfile {
    pub repo_path: String,
    pub primary_lang: PrimaryLanguage,
    pub secondary_langs: Vec<PrimaryLanguage>,
    /// Frameworks / tooling environments detected (Next.js, Vite, Docker, …)
    pub frameworks: Vec<Framework>,
    /// Build orchestration tools detected (make, just, turbo, …)
    pub build_tools: Vec<BuildTool>,
    /// Inferred code style conventions
    pub conventions: CodeConventions,
    /// True when the repo root contains multiple sub-projects
    pub monorepo: bool,
    /// 0.0–1.0 confidence in the primary language detection
    pub confidence: f32,
    /// ISO 8601 timestamp of when this profile was scanned
    pub scanned_at: String,
}
