// SPDX-License-Identifier: MIT
// Sprint N — Mailbox JSON-RPC 2.0 handlers (MR.T07, MR.T08, MR.T09, MR.T10).
//
// Registered methods (wired in ipc/mod.rs):
//   mailbox.send    — write a message to a target repo's inbox
//   mailbox.list    — list unprocessed inbox messages for a repo
//   mailbox.archive — mark a message as archived (processed)

use crate::mailbox::model::MailboxPolicy;
use crate::mailbox::storage::MailboxStorage;
use crate::AppContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

// ─── Param structs ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendParams {
    from_repo: String,
    to_repo: String,
    subject: String,
    body: String,
    reply_to: Option<String>,
    expires_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoPathParams {
    repo_path: String,
}

#[derive(Deserialize)]
struct ArchiveParams {
    id: String,
}

// ─── mailbox.send ─────────────────────────────────────────────────────────────

/// `mailbox.send` — write a cross-repo message to the target repo's inbox.
///
/// Two writes are performed atomically:
///   1. A temporary file `{uuid}.tmp` is written to `{to_repo}/.claude/inbox/`
///   2. It is renamed to `{uuid}.md` (crash-safe per MR.T09)
///
/// The message is also persisted to SQLite so `mailbox.list` can return it
/// even if the filesystem watcher has not yet fired.
pub async fn mailbox_send(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SendParams = serde_json::from_value(params)?;

    if p.from_repo.is_empty() {
        bail!("fromRepo is required");
    }
    if p.to_repo.is_empty() {
        bail!("toRepo is required");
    }
    if p.subject.is_empty() {
        bail!("subject is required");
    }
    if p.body.is_empty() {
        bail!("body is required");
    }
    validate_path("fromRepo", &p.from_repo)?;
    validate_path("toRepo", &p.to_repo)?;

    // ── Policy gate (MR.T10) ─────────────────────────────────────────────────
    // Check whether the action mentioned in the subject requires approval.
    let policy = MailboxPolicy::default();
    let subject_lc = p.subject.to_lowercase();
    let requires_approval = policy
        .require_approval
        .iter()
        .any(|action| subject_lc.contains(action.as_str()));

    if requires_approval {
        // Emit an approval-required push event; the Flutter UI shows a dialog.
        ctx.broadcaster.broadcast(
            "mailbox.approvalRequired",
            json!({
                "fromRepo": p.from_repo,
                "toRepo":   p.to_repo,
                "subject":  p.subject,
            }),
        );
        return Ok(json!({
            "queued":            false,
            "approvalRequired":  true,
            "subject":           p.subject,
        }));
    }

    // ── Write message file (atomic: .tmp → .md) ───────────────────────────────
    let storage = MailboxStorage::new(ctx.storage.clone_pool());
    let msg = storage
        .send_message(
            &p.from_repo,
            &p.to_repo,
            &p.subject,
            &p.body,
            p.reply_to.as_deref(),
            p.expires_at.as_deref(),
        )
        .await?;

    write_inbox_file(&msg)?;

    Ok(serde_json::to_value(&msg)?)
}

// ─── mailbox.list ─────────────────────────────────────────────────────────────

/// `mailbox.list` — return unarchived inbox messages for `repoPath`.
pub async fn mailbox_list(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_path("repoPath", &p.repo_path)?;

    let storage = MailboxStorage::new(ctx.storage.clone_pool());
    let messages = storage.list_messages(&p.repo_path).await?;

    Ok(json!({
        "repoPath": p.repo_path,
        "messages": serde_json::to_value(&messages)?,
        "unread":   messages.len(),
    }))
}

// ─── mailbox.archive ─────────────────────────────────────────────────────────

/// `mailbox.archive` — mark a message as archived and move its file to the
/// `{to_repo}/.claude/archive/inbox/` dead-letter directory.
pub async fn mailbox_archive(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ArchiveParams = serde_json::from_value(params)?;
    if p.id.is_empty() {
        bail!("id is required");
    }

    let storage = MailboxStorage::new(ctx.storage.clone_pool());
    storage.archive_message(&p.id).await?;

    Ok(json!({ "archived": true, "id": p.id }))
}

// ─── Inbox file writer ────────────────────────────────────────────────────────

/// Write a `MailboxMessage` to `{to_repo}/.claude/inbox/{id}.md` atomically.
///
/// Step 1: write to `{id}.tmp`
/// Step 2: `rename` to `{id}.md`  (crash-safe on POSIX; best-effort on Windows)
fn write_inbox_file(msg: &crate::mailbox::model::MailboxMessage) -> Result<()> {
    let inbox_dir = Path::new(&msg.to_repo).join(".claude/inbox");
    std::fs::create_dir_all(&inbox_dir)?;

    let tmp_path = inbox_dir.join(format!("{}.tmp", msg.id));
    let final_path = inbox_dir.join(format!("{}.md", msg.id));

    std::fs::write(&tmp_path, msg.to_markdown())?;
    std::fs::rename(&tmp_path, &final_path)?;

    Ok(())
}

// ─── Validation helper ────────────────────────────────────────────────────────

fn validate_path(field: &str, value: &str) -> Result<()> {
    if value.contains('\0') {
        bail!("invalid {field}: null byte");
    }
    if !Path::new(value).is_absolute() {
        bail!("invalid {field}: must be an absolute path");
    }
    Ok(())
}
