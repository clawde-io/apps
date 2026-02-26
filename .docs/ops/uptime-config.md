# Uptime Monitoring Configuration (Sprint UU UP.1)

## Services Monitored

| Service | URL | Type | Check Interval | Alert Threshold |
| --- | --- | --- | --- | --- |
| API (primary) | `https://api.clawde.io/health` | HTTP/HTTPS | 5 min | 2 failures |
| Web app | `https://app.clawde.io` | HTTP/HTTPS | 5 min | 2 failures |
| Marketing site | `https://clawde.io` | HTTP/HTTPS | 5 min | 2 failures |
| Admin dashboard | `https://base.clawde.io` | HTTP/HTTPS | 5 min | 2 failures |
| Registry | `https://registry.clawde.io/health` | HTTP/HTTPS | 5 min | 2 failures |
| Status page | `https://status.clawde.io` | HTTP/HTTPS | 10 min | 1 failure |

## UptimeRobot Setup

1. Create account at [uptimerobot.com](https://uptimerobot.com)
2. Add monitors for each service in the table above
3. Set alert contacts:
   - Email: `ops@clawde.io`
   - Webhook: `https://api.clawde.io/webhooks/uptime` (write to audit_log on down)
4. Set status page: public at [stats.uptimerobot.com/{YOUR_KEY}](https://stats.uptimerobot.com)
5. Share the status page URL with `status.clawde.io` via Cloudflare Worker (see UP.2)

## Hetzner Internal Probe

Hetzner server health can be monitored via API:

```bash
source ~/.claude/vault.env
curl -H "Authorization: Bearer $HETZNER_CLAWDE_TOKEN" \
  https://api.hetzner.cloud/v1/servers
```

Check `status` field — `running` is healthy.

## Escalation Path

1. UptimeRobot alert fires → email to `ops@clawde.io`
2. No response in 15 min → PagerDuty (if configured) → SMS to on-call
3. No response in 30 min → wake secondary on-call
4. If infrastructure issue: follow `incident-runbook.md`

## SLA Targets

| Tier | Uptime SLA | Measurement Window |
| --- | --- | --- |
| Free | Best effort | — |
| Personal Remote | 99% / month | Rolling 30 days |
| Cloud Basic | 99.5% / month | Rolling 30 days |
| Cloud Pro / Max | 99.9% / month | Rolling 30 days |
| Enterprise | Custom (99.99% available) | Calendar month |

## Status Page URL

Public: `https://status.clawde.io` (Cloudflare Worker, see `web/backend/workers/status/`)
