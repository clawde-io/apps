# Worktrees

Each code-modifying task in clawd gets its own isolated Git worktree. The worktree is checked out on a dedicated branch (`claw/<task-id>-<slug>`), so concurrent tasks never touch each other's in-progress changes.

---

## How It Works

When you (or the daemon) claims a task:

1. The daemon creates a new Git worktree under `{data_dir}/worktrees/<task-id>/`.
2. A branch named `claw/<task-id>-<slug>` is checked out into that directory.
3. The AI agent works exclusively inside that worktree — it cannot write to the main workspace.
4. When the task is ready for review, you call `worktrees.diff` to inspect changes.
5. You accept or reject:
   - **Accept** — squash-merges the branch into `main`, cleans up the worktree directory.
   - **Reject** — deletes the worktree and discards all changes.

```
main repo               worktrees/
  main (HEAD)             task-abc/   ← claw/task-abc-fix-login branch
  claw/task-abc-fix-login task-xyz/   ← claw/task-xyz-refactor branch
  claw/task-xyz-refactor
```

---

## Opt-In Behaviour

Worktrees are **opt-in**. By default, tasks run in the main workspace. To enable automatic worktree creation on task claim:

```toml
# clawd.toml  (or set via clawd config set worktree_mode true)
worktree_mode = true
```

When enabled, `task.claim` automatically calls `worktrees.create` before the agent starts.

If `worktree_mode = false` (default), you can still create worktrees manually via the `worktrees.create` RPC.

---

## RPC Reference

| Method | Description |
|--------|-------------|
| `worktrees.create` | Create a worktree for `task_id`. Checks out new branch `claw/<task_id>-<slug>`. |
| `worktrees.list` | List all tracked worktrees (with status: active / merged / abandoned). |
| `worktrees.diff` | Get unified diff of uncommitted changes in the worktree vs. HEAD. |
| `worktrees.commit` | Stage all changes and create a commit inside the worktree. |
| `worktrees.accept` | Squash-merge the worktree branch into `main`, emit `worktree.accepted`. |
| `worktrees.reject` | Delete the worktree and discard changes, emit `worktree.rejected`. |
| `worktrees.delete` | Hard-delete (admin/cleanup — no merge, no events). |
| `worktrees.merge` | (Legacy) merge a Done worktree — prefer `worktrees.accept`. |
| `worktrees.cleanup` | Remove all empty/stale Done worktrees. |

### worktrees.create

```json
{ "task_id": "task-abc", "task_title": "Fix login bug", "repo_path": "/home/user/myapp" }
```

Returns:

```json
{
  "task_id": "task-abc",
  "worktree_path": "/home/user/.local/share/clawd/worktrees/task-abc",
  "branch": "claw/task-abc-fix-login",
  "repo_path": "/home/user/myapp",
  "status": "Active",
  "created_at": "2026-02-24T12:00:00Z"
}
```

Push event emitted: `worktree.created { task_id, worktree_path, branch }`

### worktrees.diff

```json
{ "task_id": "task-abc" }
```

Returns:

```json
{
  "task_id": "task-abc",
  "diff": "diff --git a/src/main.rs ...\n+fn hello() {...}",
  "stats": { "files_changed": 1, "insertions": 5, "deletions": 2 }
}
```

### worktrees.accept

```json
{ "task_id": "task-abc" }
```

Returns `{ "task_id": "task-abc", "merged": true }`. Push event: `worktree.accepted { taskId, branch }`.

### worktrees.reject

```json
{ "task_id": "task-abc" }
```

Returns `{ "task_id": "task-abc", "deleted": true }`. Push event: `worktree.rejected { taskId, branch }`.

---

## Blocking Rule

If `worktree_mode = true` and a task has an **active** (unmerged) worktree, calling `task.complete` returns `worktreeNotMerged`. You must accept or reject the worktree before marking the task done.

---

## Push Events

| Event | Payload | When |
|-------|---------|------|
| `worktree.created` | `{ task_id, worktree_path, branch }` | After worktrees.create |
| `worktree.accepted` | `{ taskId, branch }` | After worktrees.accept |
| `worktree.rejected` | `{ taskId, branch }` | After worktrees.reject |

---

## Error Codes

| Code | Meaning |
|------|---------|
| `REPO_NOT_FOUND` | The `repo_path` does not exist, or no worktree exists for the given `task_id`. |
| `MERGE_CONFLICT` | `worktrees.accept` failed due to git merge conflicts. Resolve manually and retry. |

---

## Security

- Worktrees are always created inside `{data_dir}/worktrees/` — never at an arbitrary path.
- `task_id` is sanitized before use as a directory name (path traversal prevention).
- No shell execution — all git operations use the `git2` C library directly.

---

## Flutter UI

When a task has an active worktree with uncommitted changes, a `∆` badge appears on the task list item showing the number of changed files. Tapping the badge opens the **Task Diff Review** panel where you can inspect changes and accept or reject them.

The accept/reject buttons call `worktrees.accept` and `worktrees.reject` respectively and navigate back to the task list on success.
