# ClawDE — Codex AGENTS.md

This file configures Codex agent behavior for the `apps/` repository.
Governance is defined in `.claw/` — this file is a thin adapter.

## Task Work

Before editing any files, claim a task from `.claw/tasks/` using the `claim_task` tool.
All file edits require an Active+Claimed task. No freeform edits.

## Policies

- Tool risk levels: see `.claw/policies/tool-risk.json`
- Approval required for: shell commands, git push, file deletion
- Read-only tools (list, search, read): auto-approved
- Write tools (patch, tests): require Active task

## Worktrees

Each claimed task has an isolated worktree in `.claw/worktrees/<task-id>/`.
All edits happen inside the worktree. Main checkout is read-only.

## Prompts

- Control: `.claw/templates/agent-control.md`
- Planner: `.claw/templates/agent-planner.md`
- Implementer: `.claw/templates/agent-implementer.md`
- Reviewer: `.claw/templates/agent-reviewer.md`
- QA: `.claw/templates/agent-qa.md`

## Stack

This repo is Flutter + Rust only. No TypeScript, no Node.js.

- Daemon: `daemon/` — Rust/Tokio, Cargo
- Desktop: `desktop/` — Flutter
- Mobile: `mobile/` — Flutter
- Packages: `packages/` — Dart pub

Never create or modify files outside the claimed task's worktree.
