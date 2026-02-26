# Evals

ClawDE evals let you test your AI provider's capabilities against expected output patterns. Use them as a regression gate in CI or to benchmark provider quality over time.

## YAML format — `.claw/evals/*.yaml`

```yaml
- name: "file-read capability"
  prompt: "Read the file README.md and summarize it"
  expected_pattern: "read"
  pass_condition: "contains"
  provider: "claude"

- name: "git-diff capability"
  prompt: "Show me what changed in the last git diff"
  expected_pattern: "diff"
  pass_condition: "contains"
  provider: "codex"
```

### Fields

| Field | Required | Description |
| --- | --- | --- |
| `name` | Yes | Human-readable case name |
| `prompt` | Yes | The prompt sent to the AI session |
| `expected_pattern` | Yes | Pattern to check in the AI output |
| `pass_condition` | No | `contains` (default), `not_empty`, `regex` |
| `provider` | No | Provider hint — `claude`, `codex`, `auto` |

## CLI usage

```bash
# Run evals from a file and show results table
clawd eval run .claw/evals/core.yaml

# Run built-in evals
clawd eval run builtin_evals.yaml

# List eval files in the current project
clawd eval list

# CI mode — exit non-zero if pass rate < 80%
clawd eval run .claw/evals/core.yaml --ci --threshold 80
```

## CI integration

Add to `.github/workflows/ci.yml`:

```yaml
- name: Run ClawDE evals
  run: clawd eval run .claw/evals/core.yaml --ci --threshold 80
```

The `--ci` flag enables compact output and exits with code 1 if the pass rate is below the threshold.

## Built-in eval cases

ClawDE ships 10 built-in eval cases for core capabilities. Run them with:

```bash
clawd eval run builtin_evals.yaml
```

Built-in cases test: file read, file write, git diff, test run, task create, session resume, worktree create, pack install, memory inject, approval gate.

## RPC methods

| Method | Description |
| --- | --- |
| `eval.list` | List eval files in the current project |
| `eval.run` | Run an eval file, returns pass/fail/score per case |
