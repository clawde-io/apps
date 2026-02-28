//! Thin wrapper around `crate::evals::scanners::placeholders`.
//!
//! Re-exports `scan_patch` and `scan_content`, and adds
//! `check_no_placeholders` which converts the first violation into a
//! `PolicyViolation`.

pub use crate::evals::scanners::placeholders::{scan_content, scan_patch};

use super::sandbox::PolicyViolation;

/// Check that a patch contains no placeholder stubs.
///
/// Returns `Ok(())` when the patch is clean, or `Err(PolicyViolation::PlaceholderDetected)`
/// for the first violation found.
pub fn check_no_placeholders(patch: &str) -> Result<(), PolicyViolation> {
    let violations = scan_patch(patch);
    if let Some(first) = violations.into_iter().next() {
        return Err(PolicyViolation::PlaceholderDetected {
            file: first.file,
            line: first.line,
            pattern: first.pattern,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_patch_ok() {
        let patch = "\
--- a/main.rs
+++ b/main.rs
@@ -1,1 +1,2 @@
 fn main() {}
+fn helper() {}
";
        assert!(check_no_placeholders(patch).is_ok());
    }

    #[test]
    fn todo_patch_violation() {
        let patch = "\
--- a/main.rs
+++ b/main.rs
@@ -1,1 +1,3 @@
 fn main() {}
+fn todo_fn() {
+    // TODO: implement
+}
";
        let result = check_no_placeholders(patch);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(PolicyViolation::PlaceholderDetected { .. })
        ));
    }

    #[test]
    fn scan_patch_reexport_works() {
        let patch = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,1 +1,2 @@
 fn foo() {}
+fn bar() { unimplemented!() }
";
        let violations = scan_patch(patch);
        assert!(!violations.is_empty());
    }

    #[test]
    fn scan_content_reexport_works() {
        let content = "fn foo() {\n    // FIXME: broken\n}\n";
        let violations = scan_content(content, "foo.rs");
        assert!(!violations.is_empty());
    }
}
