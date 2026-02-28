# Autonomous Execution

Autonomous Execution lets ClawDE complete multi-step tasks without step-by-step approval — you set a goal, review the plan, and the AI executes until done or blocked.

## How it works

1. Set a session to Autonomous mode (toggle in session header or via `clawd session --mode autonomous`)
2. Describe the goal: "Migrate all database queries from raw SQL to the ORM"
3. The AI creates a plan (visible in the plan panel) and begins executing tasks
4. Execution continues until the goal is complete, a blocking error occurs, or context runs out
5. You review a single consolidated diff at the end

## What gets approved automatically

In Autonomous mode, the daemon approves:
- File reads
- File writes that match the stated goal
- Shell commands that are read-only (`ls`, `cat`, `cargo test`, etc.)

## What requires your approval

Even in Autonomous mode, these always pause for approval:
- Destructive shell commands (`rm`, `git reset`, `DROP TABLE`)
- Network requests outside localhost
- Commits and pushes
- Changes outside the stated repo scope

## Pause and resume

You can pause Autonomous execution at any time from the session header. The AI saves its state (current subtask, completed steps, pending diffs) and waits. Resume picks up from exactly where it left off.

## Difference from Builder Mode

| Autonomous Execution | Builder Mode |
| --- | --- |
| Single session | Multi-session graph |
| Sequential by default | Parallel subtasks |
| Goal → execute | Goal → plan review → execute |
| Best for focused tasks | Best for large feature work |

## Status

Planned for Sprint JJ alongside Builder Mode. Requires the task-graph engine and a session-level approval policy API.

## Related

- [Builder Mode](Builder-Mode.md)
- [Session Manager](Session-Manager.md)
- [Worktrees](Worktrees.md)
