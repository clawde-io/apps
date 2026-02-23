//! Secret redaction for trace events.
//!
//! Before any `TraceEvent` is written to disk, `redact_trace` should be called
//! to strip credential material.  The function scans string fields for known
//! secret patterns and high-entropy substrings, replacing matches with
//! `"[REDACTED]"`.

use once_cell::sync::Lazy;
use regex::Regex;

use super::schema::TraceEvent;

// ─── Pattern registry ─────────────────────────────────────────────────────────

/// Compiled regular expressions for known secret formats.
static SECRET_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // Anthropic / OpenAI API keys
        Regex::new(r"sk-[A-Za-z0-9\-_]{20,}").expect("regex: sk- key"),
        // GitHub personal access tokens (classic and fine-grained)
        Regex::new(r"ghp_[A-Za-z0-9]{36}").expect("regex: ghp token"),
        Regex::new(r"github_pat_[A-Za-z0-9_]{82}").expect("regex: github pat"),
        // AWS access key IDs
        Regex::new(r"AKIA[0-9A-Z]{16}").expect("regex: aws key"),
        // Generic key=value pairs (e.g. `TOKEN=abc123`)
        Regex::new(r#"(?i)(password|secret|token|api_key|auth|private_key)\s*[:=]\s*["']?[A-Za-z0-9+/\-_]{8,}"#)
            .expect("regex: key=value"),
        // PEM private key headers
        Regex::new(r"-----BEGIN\s+(?:RSA |EC |OPENSSH )?PRIVATE KEY-----")
            .expect("regex: pem header"),
        // Bearer tokens in Authorization headers
        Regex::new(r"(?i)bearer\s+[A-Za-z0-9+/\-_=]{20,}")
            .expect("regex: bearer token"),
    ]
});

// ─── Redaction helpers ────────────────────────────────────────────────────────

/// Redact secrets from a string.
///
/// Returns `(redacted_string, was_redacted)`.  If no secrets were found the
/// original string is returned unchanged.
pub fn redact_str(input: &str) -> (String, bool) {
    let mut result = input.to_string();
    let mut changed = false;

    for pat in SECRET_PATTERNS.iter() {
        if pat.is_match(&result) {
            result = pat.replace_all(&result, "[REDACTED]").to_string();
            changed = true;
        }
    }

    // Additional pass: high-entropy substrings of 20+ chars.
    let words: Vec<&str> = result.split_whitespace().collect();
    let mut rebuilt = result.clone();
    for word in &words {
        // Strip common punctuation that might be attached.
        let token = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '+' && c != '/');
        if token.len() >= 20 && is_high_entropy(token) {
            rebuilt = rebuilt.replace(token, "[REDACTED]");
            changed = true;
        }
    }
    result = rebuilt;

    (result, changed)
}

/// Detect high-entropy strings (Shannon entropy > 4.5 bits/char).
///
/// Random tokens (API keys, base64 secrets) have high entropy.
/// Natural language text does not.
pub fn is_high_entropy(s: &str) -> bool {
    if s.len() < 20 {
        return false;
    }
    let mut freq = [0u32; 256];
    let len = s.len() as f64;
    for b in s.bytes() {
        freq[b as usize] += 1;
    }
    let entropy: f64 = freq
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum();
    entropy > 4.5
}

// ─── TraceEvent redaction ─────────────────────────────────────────────────────

/// Scan and redact secrets from a `TraceEvent` in-place.
///
/// Returns `true` if any field was modified.  Sets `event.redacted = true`
/// when modifications are made so downstream consumers can identify sanitised
/// records.
pub fn redact_trace(event: &mut TraceEvent) -> bool {
    let mut any = false;

    if let Some(ref tool) = event.tool.clone() {
        let (cleaned, changed) = redact_str(tool);
        if changed {
            event.tool = Some(cleaned);
            any = true;
        }
    }

    if let Some(ref agent_id) = event.agent_id.clone() {
        let (cleaned, changed) = redact_str(agent_id);
        if changed {
            event.agent_id = Some(cleaned);
            any = true;
        }
    }

    // Redact any risk flag strings that accidentally contain secret content.
    let mut new_flags = Vec::with_capacity(event.risk_flags.len());
    for flag in &event.risk_flags {
        let (cleaned, changed) = redact_str(flag);
        if changed {
            any = true;
        }
        new_flags.push(cleaned);
    }
    event.risk_flags = new_flags;

    if any {
        event.redacted = true;
    }
    any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_openai_key() {
        let input = "sk-abcdefghijklmnopqrstuvwxyz123456";
        let (out, changed) = redact_str(input);
        assert!(changed);
        assert!(!out.contains("sk-abc"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn leaves_clean_string_unchanged() {
        let input = "tool_call bash echo hello";
        let (out, changed) = redact_str(input);
        assert!(!changed);
        assert_eq!(out, input);
    }

    #[test]
    fn high_entropy_random_string() {
        // Simulate a 32-char base64 token.
        let s = "A1B2C3D4E5F6G7H8I9J0K1L2M3N4O5P6";
        assert!(is_high_entropy(s));
    }

    #[test]
    fn low_entropy_natural_language() {
        let s = "hello world this is natural language text";
        assert!(!is_high_entropy(s));
    }
}
