# Observability

ClawDE tracks AI usage, cost, and performance in real time — giving individual developers a cost breakdown and giving Cloud admins a full-org dashboard.

## What's Tracked

| Metric | Unit | Where |
| --- | --- | --- |
| Tokens in | count | Per session tick |
| Tokens out | count | Per session tick |
| Tool calls | count | Per session tick |
| Cost (USD) | dollars | Calculated per model |

Metrics are stored locally in the daemon's SQLite database and, for Cloud users, batched to `api.clawde.io/api/metrics` every 60s.

## Desktop App

The **Cost Dashboard** (sidebar → Cost) shows:
- Session cost summary (tokens in/out, tool calls, USD)
- Token burn rate bar chart (7-day hourly)
- Tool call heatmap (24h)
- Budget warning banner (fires at 80% and 100% of configured limit)

## CLI

```bash
# 24h summary
clawd metrics summary

# List recent ticks for a session
clawd metrics list --session <id>
```

## Budget Limits

Set in `~/.claw/config.toml`:

```toml
[limits]
daily_cost_usd = 5.00
monthly_cost_usd = 100.00
```

The daemon checks against rolling totals on every tick:
- At 80%: emits `budget_warning` push event → yellow banner in app
- At 100%: emits `budget_exceeded` push event → red banner, new sessions paused

## Admin Dashboard (Cloud)

Go to **base.clawde.io → Metrics** to see:
- Aggregate cost across all users (24h KPI cards)
- Daily cost bar + cumulative line chart (7 days)
- Token burn stacked bar chart (7 days)
- Anomaly alert list with acknowledge

## Anomaly Detection

A nightly job checks if any user's 24h cost exceeds 3× their 30-day daily average. When detected:
1. A `spike` alert is created in the `metric_alerts` table
2. An email is sent to the user via Elastic Email template `cost-anomaly-alert`
3. The alert appears in the admin dashboard until acknowledged

## IPC Methods

| Method | Description |
| --- | --- |
| `metrics.list` | Recent metric ticks for a session |
| `metrics.summary` | Aggregate summary over a time window |
| `metrics.rollups` | Hourly rollups for graphing |

## Cost Model (USD per 1M tokens)

| Model | Input | Output |
| --- | --- | --- |
| Claude Opus 4.6 | $15.00 | $75.00 |
| Claude Sonnet 4.6 | $3.00 | $15.00 |
| Claude Haiku 4.5 | $0.80 | $4.00 |
| GPT-5.3 Codex | $10.00 | $30.00 |
| Codex Spark | $1.50 | $6.00 |
