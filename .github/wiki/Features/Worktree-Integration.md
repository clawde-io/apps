# Worktree Integration

ClawDE gives every code-modifying task its own isolated Git worktree. Tasks run on a dedicated branch (`claw/<id>-<slug>`) so concurrent agents never interfere with each other. When the task finishes you review the diff and choose to accept (squash-merge) or reject (discard) in one tap.

## How it works

1. **Create** — when a task starts executing, the daemon calls `worktrees.create`, which runs `git worktree add` in the project repo and records the worktree path in SQLite.
2. **Commit** — the agent commits incrementally to its own branch via `worktrees.commit`. Each commit has the task ID in the message for traceability.
3. **Diff** — at any time, `worktrees.diff` returns a unified diff of uncommitted changes plus stats (files changed, insertions, deletions).
4. **Accept** — `worktrees.accept` squash-merges the branch into the project's main branch, emits a `worktree.accepted` push event, and removes the worktree directory.
5. **Reject** — `worktrees.reject` discards all changes, emits `worktree.rejected`, and removes the worktree directory.
6. **Delete** — `worktrees.delete` is a hard-delete for admin/cleanup scenarios (no push event).

## RPC methods

| Method | Params | Returns |
| --- | --- | --- |
| `worktrees.create` | `task_id, task_title, repo_path` | `WorktreeInfo` |
| `worktrees.list` | (none) | `{ worktrees: [WorktreeInfo] }` |
| `worktrees.diff` | `task_id` | `{ diff, stats: { files_changed, insertions, deletions } }` |
| `worktrees.commit` | `task_id, message` | `{ task_id, sha }` |
| `worktrees.accept` | `task_id` | `{ task_id, merged: true }` |
| `worktrees.reject` | `task_id` | `{ task_id, deleted: true }` |
| `worktrees.delete` | `task_id` | `{ task_id, deleted: true }` |
| `worktrees.merge` | `task_id` | (legacy alias for `accept`) |
| `worktrees.cleanup` | (none) | `{ removed: N }` — prunes stale worktrees |

## Push events

| Event | Payload | When |
| --- | --- | --- |
| `worktree.created` | `{ task_id, worktree_path, branch }` | After `worktrees.create` succeeds |
| `worktree.accepted` | `{ task_id, sha }` | After squash-merge completes |
| `worktree.rejected` | `{ task_id }` | After worktree is discarded |

## Dart client (clawd_client)

```dart
// Create a worktree for a task.
final info = await client.createWorktree(taskId, taskTitle, repoPath);

// Get the unified diff.
final diff = await client.worktreeDiff(taskId);

// Commit changes inside the worktree.
await client.commitWorktree(taskId, 'feat: implement login form');

// Accept (squash-merge) or reject (discard).
await client.acceptWorktree(taskId);
await client.rejectWorktree(taskId, reason: 'approach changed');
```

## Flutter UI

The desktop app shows a **Review** badge on any task tile that has an active worktree. Tapping the badge opens the `TaskDiffReview` screen, which displays the unified diff and provides **Accept All** / **Reject All** buttons.

Merge conflicts surface as an error dialog with the conflicting file paths so the user can resolve manually and retry.

## Database

Migration `014_worktrees.sql` adds the `worktrees` table:

```
worktrees (
  id TEXT PRIMARY KEY,
  task_id TEXT,
  path TEXT,
  branch TEXT,
  base_branch TEXT,
  status TEXT,    -- active | done | abandoned | merged
  created_at INTEGER,
  merged_at INTEGER
)
```

## Safety

- Worktrees are always branched — never detached HEAD — to preserve commit history.
- `worktrees.accept` refuses to merge if there are unresolved conflicts and returns a `MERGE_CONFLICT` error code listing the affected files.
- `worktrees.cleanup` only removes worktrees whose task is in `done` or `deferred` status.
