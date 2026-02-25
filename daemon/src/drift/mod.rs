pub mod background;
/// Drift detection — compare FEATURES.md spec against source implementation.
///
/// V02.T21-T25: Identifies features marked ✅ in FEATURES.md that have no
/// corresponding source code, and stores/serves results via RPC.
pub mod scanner;
pub mod storage;

pub use scanner::{DriftItem, DriftSeverity};
