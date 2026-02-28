# Analytics

ClawDE tracks session-level usage metrics locally and surfaces them in the desktop app and web dashboard.

## What is tracked

| Metric | Where stored | Who sees it |
| --- | --- | --- |
| Token usage per session | Local SQLite | You |
| Cost estimate (input + output) | Local SQLite | You |
| Session duration | Local SQLite | You |
| Tool call counts + types | Local SQLite | You |
| Error and retry counts | Local SQLite | You |
| Aggregate monthly totals | Cloud (opt-in) | You + Team admins |

All data stays on your machine unless you are on a Cloud tier and have opted into aggregate reporting.

## Where to see it

- **Desktop app:** Settings > Analytics shows per-session and monthly summaries
- **Web dashboard** (Cloud tiers): Usage tab shows team-wide aggregates
- **CLI:** `clawd stats` prints a summary for the last 30 days

## Cost estimation

The daemon applies per-provider pricing tables (updated with each clawd release) to estimate costs. Estimates are approximate — actual charges come from your provider billing.

## Status

Basic token tracking is live in v0.1.0. Cost estimates and the Analytics UI ship in Sprint HH.

## Related

- [Daemon Reference](../Daemon-Reference.md)
- [Multi-Account](../Multi-Account.md)
