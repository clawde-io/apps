/// Coding standards library — V02.T29-T31.
///
/// Detects the primary programming language of a project by counting source
/// file extensions, then returns an embedded TOML standards bundle for that
/// language. The bundle is appended to the system prompt on session.create.
pub mod detection;
pub mod bundles;

pub use detection::detect_language;
pub use bundles::bundle_for;

/// Detected primary language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Language {
    Rust,
    TypeScript,
    Flutter, // Dart
    Python,
    Go,
    Unknown,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::Flutter => "flutter",
            Language::Python => "python",
            Language::Go => "go",
            Language::Unknown => "unknown",
        }
    }
}
