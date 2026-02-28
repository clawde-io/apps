# GitHub App Integration

The ClawDE GitHub App connects your repositories to the `clawd` daemon. It automatically runs code review on pull requests and posts CI results as commit status checks.

## Installation

1. Go to **base.clawde.io → Integrations → GitHub**
2. Click **Install GitHub App**
3. Choose the repositories to grant access
4. ClawDE will confirm the webhook is active

## What It Does

### PR Code Review

When a pull request is opened or synchronized, ClawDE:

1. Fetches the PR diff via the GitHub API
2. Creates a daemon session with the diff as context
3. Runs the built-in `code-review` workflow
4. Posts a GitHub PR review comment with findings

### CI Status Checks

On every push to a tracked branch, ClawDE:

1. Triggers `clawd ci run` against the daemon API
2. Streams build and test results
3. Posts a **ClawDE CI** commit status check (pending → success/failure)

## Setup

The GitHub App requires three secrets in your backend environment:

| Variable | Description |
| --- | --- |
| `GITHUB_APP_ID` | Your GitHub App ID (from App settings) |
| `GITHUB_APP_PRIVATE_KEY` | PEM private key for the App |
| `GITHUB_WEBHOOK_SECRET` | Secret used to verify webhook payloads |

Add these to `~/.claude/vault.env` and reference them in the backend `.env.prod`.

## Webhook Endpoint

Incoming webhooks are received at:

```
POST https://api.clawde.io/webhooks/github
```

Payloads are verified using HMAC-SHA256 with the `GITHUB_WEBHOOK_SECRET`.

## Source

- Webhook handler: [`site/backend/services/clawde-api/services/github-app/`](https://github.com/clawde-io/web)
- Admin UI: [`site/base/src/app/(dashboard)/integrations/github/`](https://github.com/clawde-io/web)
