# Incident Runbook (Sprint UU UP.3)

## Severity Levels

| Level | Definition | Response Time | Examples |
| --- | --- | --- | --- |
| P0 — Critical | Full service outage; all users affected | < 15 min | API completely down; database unreachable |
| P1 — High | Major feature broken; many users affected | < 1 hour | Authentication failure; billing broken |
| P2 — Medium | Degraded performance or partial outage | < 4 hours | Slow AI responses; relay intermittent |
| P3 — Low | Minor impact; workaround available | < 24 hours | Occasional errors; non-critical feature broken |

## On-Call

- Primary: `ops@clawde.io`
- Escalation (P0 only): founders
- UptimeRobot sends alerts to `ops@clawde.io` first

## P0 Response Steps

### 1. Acknowledge (< 5 min)
```
Post in #incidents Slack channel:
"P0 OPEN — [brief description] — investigating"
```

### 2. Assess (< 10 min)
```bash
# Check server health
ssh root@159.69.190.92 "nself status"

# Check all services
curl https://api.clawde.io/health
curl https://registry.clawde.io/health

# Check logs
ssh root@159.69.190.92 "nself logs --tail 100"
```

### 3. Mitigate

**Scenario: API unreachable**
```bash
ssh root@159.69.190.92
nself restart clawde-api
# If still down:
nself restart
```

**Scenario: Database corrupt / migration failed**
```bash
ssh root@159.69.190.92
# Check Postgres health
nself db status
# Restore from backup if needed (Hetzner snapshots)
```

**Scenario: Hetzner server unreachable**
1. Check Hetzner console: https://console.hetzner.cloud/projects
2. Hard-reset via Hetzner API if needed
3. If catastrophic: provision new cx22 from snapshot backup

**Scenario: Vercel deployment broken**
```bash
source ~/.claude/vault.env
# Roll back to previous deployment
vercel rollback --scope $VERCEL_TEAM_ID clawde-site
vercel rollback --scope $VERCEL_TEAM_ID clawde-base
vercel rollback --scope $VERCEL_TEAM_ID clawde-app
```

### 4. Communicate (within 30 min of P0 start)
Update `status.clawde.io` by calling the worker's internal update endpoint:
```bash
curl -X POST https://status.clawde.io/update \
  -H "x-internal-token: $CLAWD_INTERNAL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"service":"api","ok":false}'
```

For extended outages (> 30 min), send user email via Elastic Email.

### 5. Resolve
- Fix the root cause
- Restore all affected services
- Update status page to `ok`
- Write post-mortem within 48 hours

## Post-Mortem Template

```markdown
# Incident Post-Mortem — [YYYY-MM-DD]

**Severity:** P0 / P1 / P2
**Duration:** HH:MM
**Services Affected:**
**Users Affected:**

## Timeline
- HH:MM UTC — Alert triggered
- HH:MM UTC — Acknowledged
- HH:MM UTC — Root cause identified
- HH:MM UTC — Mitigation applied
- HH:MM UTC — Resolved

## Root Cause

## Contributing Factors

## Resolution

## Action Items
| Item | Owner | Due Date |
| --- | --- | --- |

## Lessons Learned
```

## Backup Procedures

### SQLite Daemon Data
Daemon auto-backs up SQLite before migrations to `~/.clawd/backups/`.

### Hetzner Postgres
Hetzner provides automatic daily snapshots (7-day retention on Managed DB).
Manual snapshot before any major migration:
```bash
# Via Hetzner API
source ~/.claude/vault.env
curl -X POST \
  "https://api.hetzner.cloud/v1/volumes/YOUR_VOLUME_ID/actions/create_snapshot" \
  -H "Authorization: Bearer $HETZNER_CLAWDE_TOKEN"
```

## Contacts

| Role | Contact |
| --- | --- |
| On-call | ops@clawde.io |
| Hetzner support | https://www.hetzner.com/support |
| Vercel support | https://vercel.com/support |
| Stripe support | https://support.stripe.com |
