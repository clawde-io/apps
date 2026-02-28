# CI Runner

> Sprint EE — Embedded CI/CD runner and GitHub Actions integration.

## Overview

ClawDE's CI Runner lets you run AI-assisted CI pipelines locally or in GitHub Actions. Each step is an AI task executed by `clawd`, with real-time status broadcast to connected clients.

## Architecture

```
.claw/ci.yaml        ← pipeline definition
daemon (ci.* RPCs)   ← run / status / cancel
GitHub Action        ← clawde-io/clawd-action@v1
```

## Pipeline Definition

Create `.claw/ci.yaml` in your repository:

```yaml
name: AI Code Review
on:
  - pull_request
steps:
  - name: lint
    run: "Review the changed files for lint issues and code style violations."
    provider: claude
  - name: security
    run: "Check for any security vulnerabilities in the diff."
    provider: codex
  - name: summary
    run: "Write a concise review summary with pass/fail verdict."
    provider: claude
```

### Fields

| Field | Type | Description |
| --- | --- | --- |
| `name` | string | Pipeline display name |
| `on` | string[] | Trigger events (`pull_request`, `push`, `manual`) |
| `steps[].name` | string | Step identifier |
| `steps[].run` | string | AI prompt for this step |
| `steps[].provider` | string | AI provider (`claude`, `codex`) — default `claude` |

## RPC Methods

| Method | Description |
| --- | --- |
| `ci.run` | Start a CI pipeline, returns `runId` |
| `ci.status` | Get current run status and step results |
| `ci.cancel` | Cancel a running pipeline |

### ci.run

```json
{ "repo_path": "/path/to/repo", "trigger": "pull_request" }
```

Returns: `{ "runId": "...", "status": "running" }`

Push event `ci.stepStarted` fires when each step begins; `ci.complete` fires when the pipeline finishes.

## GitHub Actions

Use the official ClawDE action in your workflow:

```yaml
# .github/workflows/ai-review.yml
name: AI Code Review
on: [pull_request]
jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: clawde-io/clawd-action@v1
        with:
          task: "Review this pull request for bugs and style issues."
          github-token: ${{ secrets.GITHUB_TOKEN }}
          post-comment: true
```

### Action Inputs

| Input | Required | Default | Description |
| --- | --- | --- | --- |
| `task` | Yes | — | AI prompt to run |
| `github-token` | No | — | GitHub token for posting comments |
| `repo-path` | No | `.` | Repository path |
| `step` | No | — | Specific step name to run |
| `post-comment` | No | `false` | Post AI output as PR comment |

## Desktop Integration

The CI panel in the desktop app shows real-time step progress. Open it from the sidebar when a CI run is active.

## CLI

```bash
# Start a CI run
clawd ci run --repo /path/to/repo --trigger pull_request

# Check status
clawd ci status <runId>
```
