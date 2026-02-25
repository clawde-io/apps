# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in ClawDE, report it privately.

**Do not** open a public GitHub issue for security vulnerabilities.

**Report to:** <security@clawde.io>

Include in your report:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested fix (optional)

**Response timeline:**
- Acknowledgement within 48 hours
- Initial assessment within 5 business days
- Fix and coordinated disclosure within 90 days

## Scope

**In scope:**
- `clawd` daemon RPC interface (WebSocket, port 4300)
- Relay connection protocol and E2E encryption
- Pack signing and verification
- Authentication and authorization (local + relay)
- SQLite data integrity

**Out of scope:**
- Physical access to the host machine
- Vulnerabilities in AI provider CLIs (Claude Code, Codex) — report to those projects
- Denial-of-service requiring physical proximity
- Social engineering

## Supported Versions

| Version | Supported |
|---------|-----------|
| Latest release | Yes |
| Older releases | No — update to latest |

## Disclosure Policy

We follow coordinated disclosure with a 90-day deadline. We will credit researchers in the release notes unless they prefer to remain anonymous.

## Security Release Process

Security fixes ship as patch releases (e.g., v0.1.0 → v0.1.1). Release notes include the CVE number and a summary of the impact. Critical fixes ship within 7 days of confirmation.
