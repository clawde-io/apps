# Ghost Diff

Ghost Diff detects when AI session file changes diverge from your spec files (`.claw/specs/`). It acts as a soft guardrail — the daemon fires a `ghost_drift_detected` push event and the UI shows a warning banner before drift compounds.

## Spec file format — `.claw/specs/{component}.md`

Write plain markdown with an "Expected behavior" section:

```markdown
# Session Module Spec

## Expected behavior

- Sessions must persist across daemon restarts using SQLite storage
- Session IDs must be globally unique (UUID v4)
- Sessions time out after 30 minutes of inactivity
- All session operations are atomic (no partial writes)
```

The ghost diff engine extracts bullet points from "Expected behavior", "Specification", and "Requirements" sections. Files elsewhere in the document are used if no section is found.

## CLI usage

```bash
# Run ghost diff check in terminal
clawd ghost-diff

# Exit non-zero on any drift (CI integration)
clawd ghost-diff --strict
```

## CI integration

```yaml
- name: Ghost diff check
  run: clawd ghost-diff --strict
```

Exits with code 1 if any spec violations are detected.

## Push event

When the daemon detects drift in real-time (file saved → spec check fails), it fires:

```json
{
  "method": "ghost_drift_detected",
  "params": {
    "file": "src/session.rs",
    "spec": "session.md",
    "divergenceSummary": "File may diverge from spec: missing concepts: persist, atomic",
    "severity": "medium"
  }
}
```

## Flutter Ghost Diff panel

The Ghost Diff panel (in Session → Details) shows:

- List of drift warnings with spec name + file path
- Spec snippet vs actual code side-by-side
- "Accept Drift" button — updates the spec to match current code
- "Revert" button — opens git diff view to undo the file change

## RPC method

| Method | Description |
| --- | --- |
| `ghost_diff.check` | Run ghost diff for a repo, returns list of `GhostDriftWarning` |

## How it works

1. Get files changed since last git commit (`git diff HEAD --name-only`)
2. Load all spec files from `.claw/specs/`
3. Match each changed file against relevant specs by name
4. Extract keywords from "Expected behavior" sections
5. Check that keyword concepts appear in the file content
6. Flag files where more than half the spec concepts are absent
