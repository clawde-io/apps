// security/ — Security utilities, prompt injection defense + content labeling
//
// This module consolidates the original security utilities (path safety, tool
// gating, input sanitization) with the Sprint ZZ injection defense additions.

pub mod content_labels;
pub mod injection_eval;

// Re-export utilities from the pre-ZZ security.rs (now merged here).
pub use guard::{
    check_repo_path_safety, check_tool_call, normalize_path, safe_path, sanitize_tool_input,
    strip_null_bytes, validate_session_id,
};

pub mod guard;
