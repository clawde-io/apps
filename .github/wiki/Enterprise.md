# Enterprise Deployment Guide

This guide covers deploying ClawDE in an enterprise environment. Enterprise features require an Enterprise subscription (custom pricing — [contact us](https://clawde.io/enterprise)).

## What is included in Enterprise

| Feature | Available |
| --- | --- |
| On-prem relay | Yes — run the relay on your infrastructure |
| SAML 2.0 SSO | Yes — Okta, Azure AD, Google Workspace, any SAML 2.0 IdP |
| OIDC SSO | Yes — any OpenID Connect provider |
| SSO enforcement | Yes — block password login, require IdP |
| RBAC | Yes — Admin, Developer, Reviewer, Viewer roles |
| Audit log | Yes — immutable, exportable (JSON + CSV) |
| Provider policy | Yes — restrict which AI providers and models may be used |
| Path restrictions | Yes — restrict which filesystem paths sessions can access |
| Approval gates | Yes — require human approval for high-risk tool calls |
| SLA | 99.9% uptime (cloud-hosted), 99.5% (self-hosted relay support) |
| Custom contracts | MSA, DPA, custom invoicing, purchase orders |
| Priority support | Dedicated Slack channel + named support engineer |

---

## Architecture options

### Option A: ClawDE Cloud (recommended)

The daemon runs on ClawDE infrastructure. You connect your enterprise IdP and policies via the admin portal at `app.clawde.io`. No self-hosted components required.

```
Developers  ──→  Desktop/Mobile app  ──→  Relay (api.clawde.io)  ──→  Cloud daemon
                                                                          │
                                                                     Postgres audit log
                                                                     Hasura GraphQL
```

### Option B: On-prem relay with ClawDE Cloud daemon

You run the relay on your own infrastructure (VPN or data centre). Traffic never leaves your network boundary before reaching the cloud daemon over a private channel. Requires Enterprise plan.

```
Developers  ──→  Desktop/Mobile app  ──→  On-prem relay (your infra)  ──→  ClawDE Cloud daemon
```

### Option C: Fully self-hosted (Enterprise Self-Hosted)

You run the relay AND the daemon on your infrastructure. Available for Enterprise customers with specific data-residency requirements.

```
Developers  ──→  Desktop/Mobile app  ──→  On-prem relay (your infra)  ──→  On-prem daemon (your infra)
```

Contact [enterprise@clawde.io](mailto:enterprise@clawde.io) for the self-hosted daemon setup guide (not part of the open-source distribution).

---

## SSO configuration

### Supported identity providers

| Provider | Protocol | Notes |
| --- | --- | --- |
| Okta | SAML 2.0 or OIDC | Both protocols tested |
| Azure Active Directory | SAML 2.0 or OIDC | Use "Enterprise App" registration |
| Google Workspace | SAML 2.0 or OIDC | |
| Ping Identity | SAML 2.0 | |
| OneLogin | SAML 2.0 | |
| Auth0 | OIDC | |
| Any SAML 2.0 IdP | SAML 2.0 | Must support HTTP-POST binding |
| Any OIDC provider | OIDC | Must support authorization_code flow |

### SAML 2.0 setup

1. In your IdP, create a new SAML application.

2. Configure the following:

   | Field | Value |
   | --- | --- |
   | ACS URL | `https://api.clawde.io/sso/saml/callback/{your_org_id}` |
   | Entity ID / Audience | `https://api.clawde.io/sso/saml/metadata` |
   | Name ID format | `EmailAddress` |
   | Attribute mapping | `email` → `user.email`, `first_name` → `user.firstName` |

3. In the ClawDE admin portal (`app.clawde.io/enterprise`), go to **SSO** tab.

4. Select **Configure SAML 2.0** and enter:
   - IdP Entity ID
   - SSO redirect URL
   - X.509 certificate (PEM)

5. Test the connection. Once confirmed, enable **Enforce SSO** to require all org members to sign in via the IdP.

### OIDC setup

1. In your IdP, register a new OAuth 2.0 / OIDC application.

2. Set the redirect URI to:
   ```
   https://api.clawde.io/sso/oidc/callback/{your_org_id}
   ```

3. Note the **client ID** and **client secret**.

4. In the ClawDE admin portal, go to **SSO** tab → **Configure OIDC** and enter:
   - Issuer URL (the IdP discovery endpoint, e.g. `https://accounts.google.com`)
   - Client ID
   - Client secret
   - Redirect URI (pre-filled)

5. Test and enable.

---

## Policy management

Enterprise orgs can restrict what AI providers, models, and file paths are allowed in sessions, and can require human approval for high-risk tool calls.

Policies are managed in the **Policies** tab of `app.clawde.io/enterprise` or via the API.

### Policy types

#### provider_allowlist

Restrict sessions to specific AI providers.

```json
{
  "policy_type": "provider_allowlist",
  "config_json": {
    "providers": ["claude", "codex"]
  }
}
```

#### model_allowlist

Restrict which model IDs may be used.

```json
{
  "policy_type": "model_allowlist",
  "config_json": {
    "models": ["claude-opus-4", "gpt-4o"]
  }
}
```

#### path_restriction

Limit which filesystem paths sessions can access. All access outside the listed paths is denied.

```json
{
  "policy_type": "path_restriction",
  "config_json": {
    "allowed_paths": ["/workspace/myproject", "/workspace/shared"],
    "deny_outside": true
  }
}
```

#### approval_required

Require human approval for tool calls that match the policy (listed tools or risk threshold).

```json
{
  "policy_type": "approval_required",
  "config_json": {
    "tools": ["bash", "file_write"],
    "risk_threshold": "medium"
  }
}
```

Risk levels: `low` → `medium` → `high` → `critical`. The threshold means "require approval if risk is at least this level".

### Policy precedence

Policies are evaluated in order of creation (oldest first). The first **deny** wins. Policies apply to all sessions in the org regardless of which user or agent created them.

---

## Audit log

The audit log records all significant actions: session create/delete, tool calls, admin changes, auth events, billing events, and policy evaluations.

The log is append-only and immutable — no entry can be modified or deleted.

### Export

In the **Audit log** tab of the enterprise portal:

1. Set the date range.
2. Click **Export JSON** or **Export CSV**.
3. The export downloads up to 5,000 entries per request.

For large exports, use the API with pagination:

```sh
# First page
curl -H "Authorization: Bearer <token>" \
  "https://api.clawde.io/api/enterprise/audit-log?org_id=my-org&from=2026-01-01T00:00:00Z&to=2026-02-01T00:00:00Z&limit=500&format=json"

# Next page (use the `id` of the last entry as cursor)
curl -H "Authorization: Bearer <token>" \
  "https://api.clawde.io/api/enterprise/audit-log?org_id=my-org&from=2026-01-01T00:00:00Z&to=2026-02-01T00:00:00Z&limit=500&cursor=<last_id>"
```

### Retention

Audit log entries are retained for 2 years by default. Contact support for custom retention.

---

## RBAC

Enterprise orgs have four built-in roles:

| Role | Permissions |
| --- | --- |
| **Admin** | Full access — policies, SSO, billing, audit, user management |
| **Developer** | Create sessions, tasks, repos; use all AI providers |
| **Reviewer** | Read sessions and tasks; approve/deny tool calls and worktrees |
| **Viewer** | Read-only access to sessions, tasks, and analytics |

Roles are assigned per org member in `app.clawde.io/enterprise/members` or via the API.

---

## Network requirements

### Outbound (from developer machines)

| Destination | Port | Protocol | Purpose |
| --- | --- | --- | --- |
| `api.clawde.io` | 443 | HTTPS/WSS | Relay and license verification |
| AI provider APIs | 443 | HTTPS | Claude, OpenAI, etc. (direct from clawd) |

### Firewall notes

- The daemon connects **outbound only** — no inbound ports required on developer machines.
- The relay uses standard WebSocket over TLS (port 443) — no custom ports.
- For on-prem relay deployments, you control the network path.

---

## On-prem relay setup

Enterprise customers can run the relay server on their own infrastructure.

### Requirements

- Linux server (Ubuntu 22.04+ or Debian 12+)
- Docker 24+
- A domain with a valid TLS certificate
- Outbound HTTPS to `api.clawde.io` for license verification

### Deployment

Contact [enterprise@clawde.io](mailto:enterprise@clawde.io) for the on-prem relay Docker image and configuration guide.

The relay image is not part of the open-source distribution.

---

## Support

Enterprise customers receive:

- Dedicated Slack connect channel
- Named support engineer
- 4-hour response SLA for critical issues (P1)
- 1-business-day response for non-critical issues

Email: [enterprise@clawde.io](mailto:enterprise@clawde.io)

---

## See also

- [Getting Started](Getting-Started.md)
- [Architecture](Architecture.md)
- [Security](Security.md)
- [RPC Reference](RPC-Reference.md)
