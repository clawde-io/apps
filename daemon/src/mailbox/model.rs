// SPDX-License-Identifier: MIT
// Sprint N — Mailbox data model (MR.T06, MR.T07, MR.T10).

use serde::{Deserialize, Serialize};

// ─── MailboxMessage ───────────────────────────────────────────────────────────

/// A cross-repo inbox message.  Messages are written as Markdown files into
/// `{to_repo}/.claude/inbox/{uuid}.md` and mirrored into SQLite for fast
/// querying and archival.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxMessage {
    pub id:         String,
    pub from_repo:  String,
    pub to_repo:    String,
    pub subject:    String,
    pub body:       String,
    /// Optional reply-to path — where the recipient should send a response.
    pub reply_to:   Option<String>,
    /// RFC 3339 timestamp after which the message is dead-lettered.
    pub expires_at: Option<String>,
    /// True when the message has been processed and archived.
    pub archived:   bool,
    pub created_at: String,
}

impl MailboxMessage {
    /// Render the message as a Markdown file suitable for writing to
    /// `{to_repo}/.claude/inbox/{id}.md`.
    ///
    /// Format matches the GCI inbox protocol so human-readable and parseable.
    pub fn to_markdown(&self) -> String {
        let mut out = String::from("---\n");
        out.push_str(&format!("id: {}\n", self.id));
        out.push_str(&format!("from: {}\n", self.from_repo));
        out.push_str(&format!("to: {}\n", self.to_repo));
        out.push_str(&format!("subject: {}\n", self.subject));
        if let Some(ref rt) = self.reply_to {
            out.push_str(&format!("reply_to: {rt}\n"));
        }
        if let Some(ref ea) = self.expires_at {
            out.push_str(&format!("expires_at: {ea}\n"));
        }
        out.push_str(&format!("created_at: {}\n", self.created_at));
        out.push_str("---\n\n");
        out.push_str(&self.body);
        out.push('\n');
        out
    }

    /// Parse a MailboxMessage from a Markdown file that contains a YAML
    /// front-matter block (lines between `---` delimiters) followed by the
    /// body text.
    ///
    /// Returns `None` if the front-matter is missing or malformed.
    pub fn from_markdown(content: &str, file_id: &str) -> Option<MailboxMessage> {
        // Locate front-matter block.
        let content = content.trim();
        if !content.starts_with("---") {
            return None;
        }
        let after_first = &content[3..];
        let end = after_first.find("\n---")?;
        let front_matter = &after_first[..end];
        let body_start   = end + 4; // skip "\n---"
        let body = after_first
            .get(body_start..)
            .unwrap_or_default()
            .trim()
            .to_string();

        // Parse YAML key: value lines (simple subset only).
        let mut fields: std::collections::HashMap<&str, &str> =
            std::collections::HashMap::new();
        for line in front_matter.lines() {
            if let Some((k, v)) = line.split_once(": ") {
                fields.insert(k.trim(), v.trim());
            }
        }

        let id       = fields.get("id").map(|s| s.to_string()).unwrap_or_else(|| file_id.to_string());
        let from_repo = fields.get("from")?.to_string();
        let to_repo   = fields.get("to")?.to_string();
        let subject   = fields.get("subject")?.to_string();
        let created_at = fields
            .get("created_at")
            .map(|s| s.to_string())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        Some(MailboxMessage {
            id,
            from_repo,
            to_repo,
            subject,
            body,
            reply_to:   fields.get("reply_to").map(|s| s.to_string()),
            expires_at: fields.get("expires_at").map(|s| s.to_string()),
            archived:   false,
            created_at,
        })
    }
}

// ─── MailboxPolicy ────────────────────────────────────────────────────────────

/// Per-repo policy controlling which cross-repo operations require explicit
/// user approval before they are executed (MR.T10).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxPolicy {
    /// Action types that must be explicitly approved before execution.
    /// Examples: `"cross-repo-write"`, `"deploy"`.
    pub require_approval: Vec<String>,
}

impl Default for MailboxPolicy {
    fn default() -> Self {
        MailboxPolicy {
            require_approval: vec!["cross-repo-write".to_string(), "deploy".to_string()],
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg() -> MailboxMessage {
        MailboxMessage {
            id: "test-id-1".to_string(),
            from_repo: "apps".to_string(),
            to_repo: "web".to_string(),
            subject: "Test subject".to_string(),
            body: "Hello from apps to web.".to_string(),
            reply_to: None,
            expires_at: None,
            archived: false,
            created_at: "2026-02-25T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn to_markdown_contains_front_matter() {
        let msg = make_msg();
        let md = msg.to_markdown();
        assert!(md.starts_with("---\n"), "should start with ---");
        assert!(md.contains("id: test-id-1"));
        assert!(md.contains("from: apps"));
        assert!(md.contains("to: web"));
        assert!(md.contains("subject: Test subject"));
        assert!(md.contains("Hello from apps to web."));
    }

    #[test]
    fn from_markdown_roundtrip() {
        let msg = make_msg();
        let md = msg.to_markdown();
        let parsed = MailboxMessage::from_markdown(&md, "test-id-1")
            .expect("should parse back");
        assert_eq!(parsed.id, "test-id-1");
        assert_eq!(parsed.from_repo, "apps");
        assert_eq!(parsed.to_repo, "web");
        assert_eq!(parsed.subject, "Test subject");
        assert_eq!(parsed.body, "Hello from apps to web.");
    }

    #[test]
    fn from_markdown_with_reply_to() {
        let mut msg = make_msg();
        msg.reply_to = Some("~/Sites/apps/.claude/inbox/".to_string());
        let md = msg.to_markdown();
        let parsed = MailboxMessage::from_markdown(&md, "test-id-1").unwrap();
        assert_eq!(
            parsed.reply_to.as_deref(),
            Some("~/Sites/apps/.claude/inbox/")
        );
    }

    #[test]
    fn from_markdown_returns_none_without_front_matter() {
        let result = MailboxMessage::from_markdown("just plain text\nno front matter", "file-id");
        assert!(result.is_none());
    }

    #[test]
    fn from_markdown_returns_none_missing_required_fields() {
        // Missing `from:` and `to:` fields
        let bad_md = "---\nid: x\nsubject: Hello\n---\n\nBody text\n";
        let result = MailboxMessage::from_markdown(bad_md, "x");
        assert!(result.is_none());
    }

    #[test]
    fn mailbox_policy_default_requires_cross_repo_write_and_deploy() {
        let policy = MailboxPolicy::default();
        assert!(policy.require_approval.contains(&"cross-repo-write".to_string()));
        assert!(policy.require_approval.contains(&"deploy".to_string()));
    }

    #[test]
    fn to_markdown_includes_expires_at_when_set() {
        let mut msg = make_msg();
        msg.expires_at = Some("2026-12-31T23:59:59Z".to_string());
        let md = msg.to_markdown();
        assert!(md.contains("expires_at: 2026-12-31T23:59:59Z"));
    }
}
