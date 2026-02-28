/// Repo Intelligence — stack detection, artifact generation, drift scoring,
/// and validator auto-derivation (Sprint F, RI.T01–T18).
pub mod artifacts;
pub mod drift;
pub mod profile;
pub mod scanner;
pub mod storage;
pub mod validator;

pub use profile::{BuildTool, CodeConventions, Framework, PrimaryLanguage, RepoProfile};

/// Convenience re-export: run a full scan on a repo path.
///
/// This is the entry point called by the `repo.scan` RPC handler.
/// Wrapped in `spawn_blocking` by the caller (scanner ops are sync I/O).
pub use scanner::scan;
