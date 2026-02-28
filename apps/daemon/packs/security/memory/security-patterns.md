# Security Conventions

## Input Validation
- Never trust user input — validate at every boundary (API, form, CLI)
- Parameterized queries only — no string concatenation in SQL
- Sanitize HTML before rendering — use DOMPurify or equivalent
- Validate file uploads: MIME type, extension, size, content

## Authentication
- Tokens: minimum 32 bytes entropy; use `crypto.randomBytes` (Node) or `secrets.token_bytes` (Python)
- Passwords: bcrypt/argon2 — never SHA-256 or MD5 for passwords
- JWTs: short expiry (15 min access, 7 day refresh); store refresh tokens in httpOnly cookies
- Session fixation: regenerate session ID after privilege elevation

## Secrets Management
- Never hardcode secrets in source code or commit them
- Load from env vars at runtime; never log env vars
- Rotate keys if they are ever exposed — assume compromised

## Common Vulnerabilities
- XSS: use CSP headers + escape output — never `innerHTML` with user content
- CSRF: SameSite cookies + CSRF token for state-mutating endpoints
- Path traversal: resolve paths with `path.resolve` and check they stay within allowed root
- SSRF: allowlist outbound URLs — never proxy arbitrary user-supplied URLs

## Dependencies
- `npm audit --audit-level=high` in CI — fail on high/critical
- Review new deps: last commit date, CVE history, license
- Pin major versions; use lockfiles always
