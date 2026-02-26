# Prompt Intelligence

Prompt Intelligence is the layer that automatically selects, compresses, and enriches the context sent to the AI on every turn — so you get better results without manually managing what goes into the prompt.

## What it does

### Context selection
The daemon scores each file in the repo for relevance to the current message:
- Files you recently edited get higher weight
- Files referenced by name in your message are always included
- Files with open diagnostics are included when the task involves fixing errors
- Files outside the active task scope are excluded

### Context compression
When the conversation grows past the context budget, older turns are summarized rather than truncated. The summary preserves:
- Key decisions made in prior turns
- File paths that were modified
- Error messages that were resolved

### Prompt enrichment
Before sending to the AI, the daemon appends:
- Current git status (modified files, current branch)
- Active diagnostic errors from LSP (when configured)
- Repo structure summary for large codebases

## Budget indicator

The desktop app shows a context budget bar (bottom of chat) that turns amber at 70% and red at 90%. Hovering shows a breakdown of what's consuming the budget.

## Status

Context selection and the budget bar are live in v0.1.0. Compression and LSP enrichment ship with Sprint II.

## Related

- [LSP Integration](LSP.md)
- [Repo Intelligence](Repo-Intelligence.md)
- [Session Manager](Session-Manager.md)
