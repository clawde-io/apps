//! Output scanning — secret redaction and untrusted content labelling.
//!
//! Applied after a tool returns its result and before the output is displayed
//! or stored in the event log.  Wraps `crate::evals::scanners::secrets` and
//! `crate::telemetry::redact`.

use crate::evals::scanners::secrets as secret_scanner;
use crate::telemetry::redact::redact_str;

use super::sandbox::PolicyViolation;

// ─── Untrusted content label ──────────────────────────────────────────────────

/// Wraps a string value with a flag indicating whether its origin is untrusted
/// (e.g. tool output from an external MCP server).
#[derive(Debug, Clone)]
pub struct UntrustedContentLabel {
    /// The (possibly redacted) content.
    pub content: String,
    /// `true` when this content comes from an untrusted source and should be
    /// treated with caution before display or further processing.
    pub from_untrusted: bool,
}

impl UntrustedContentLabel {
    pub fn trusted(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            from_untrusted: false,
        }
    }

    pub fn untrusted(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            from_untrusted: true,
        }
    }
}

// ─── Patch / diff scanning ────────────────────────────────────────────────────

/// Scan a patch diff for secrets.
///
/// Returns a list of `PolicyViolation::SecretDetected` entries — one for each
/// added line in the diff that contains a credential.  Uses the same pattern
/// set as `evals::scanners::secrets::scan_patch`.
pub fn scan_patch_output(patch: &str) -> Vec<PolicyViolation> {
    secret_scanner::scan_patch(patch)
        .into_iter()
        .map(|v| PolicyViolation::SecretDetected {
            location: format!("{}:{}", v.file, v.line),
            detail: format!("[{}] {}", v.secret_type, v.redacted_preview),
        })
        .collect()
}

// ─── Log / display text redaction ────────────────────────────────────────────

/// Redact secrets from a log or display string.
///
/// Uses `telemetry::redact::redact_str` for the redaction logic.
/// Returns the redacted string (unchanged if no secrets were found).
pub fn scan_log_output(text: &str) -> String {
    let (redacted, _changed) = redact_str(text);
    redacted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_patch_finds_api_key() {
        let patch = "\
--- a/config.rs
+++ b/config.rs
@@ -1,1 +1,2 @@
 fn config() {}
+const KEY: &str = \"sk-abcdefghijklmnopqrstuvwxyz1234567890\";
";
        let violations = scan_patch_output(patch);
        assert!(!violations.is_empty());
        assert!(matches!(
            &violations[0],
            PolicyViolation::SecretDetected { .. }
        ));
    }

    #[test]
    fn clean_patch_no_violations() {
        let patch = "\
--- a/main.rs
+++ b/main.rs
@@ -1,1 +1,2 @@
 fn main() {}
+fn helper() { println!(\"hello\"); }
";
        let violations = scan_patch_output(patch);
        assert!(violations.is_empty());
    }

    #[test]
    fn scan_log_redacts_token() {
        let input = "calling api with sk-abcdefghijklmnopqrstuvwxyz123456";
        let output = scan_log_output(input);
        assert!(!output.contains("sk-abc"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn untrusted_label_flag() {
        let label = UntrustedContentLabel::untrusted("some output");
        assert!(label.from_untrusted);

        let safe = UntrustedContentLabel::trusted("safe output");
        assert!(!safe.from_untrusted);
    }
}
