# Builder Mode

Builder Mode is a structured execution mode where you define a goal and ClawDE breaks it into sub-tasks, assigns them to sessions, and executes them in dependency order — all without step-by-step prompting.

## How it differs from a regular session

| Regular session | Builder Mode |
| --- | --- |
| One-shot prompts | Goal decomposition |
| Manual follow-up | Automatic subtask chaining |
| Single session | Parallel sessions (one per subtask) |
| You review each step | You review the final diff |

## Workflow

1. Describe a goal: "Add OAuth login to the web app"
2. ClawDE generates a task tree (plan phase)
3. You approve or edit the tree
4. Execution begins — sessions run in parallel where possible
5. Each subtask produces a diff; Builder Mode assembles the final changeset
6. One review and approve

## Guardrails

Builder Mode never force-pushes or merges without your final approval. Each subtask diff is visible individually before the combined change is applied.

## Status

Planned for Sprint JJ. Requires task graph engine in the daemon + Builder UI in the desktop app.

## Related

- [Session Manager](Session-Manager.md)
- [Autonomous Execution](Autonomous-Execution.md)
- [Worktrees](Worktrees.md)
