# Git Conventions

## Conventional Commits
Format: `<type>(<scope>): <description>`

Types:
- `feat`: new feature
- `fix`: bug fix
- `docs`: documentation only
- `style`: formatting (no logic change)
- `refactor`: restructuring without feature/fix
- `test`: adding or fixing tests
- `chore`: build, tooling, deps (no production code change)
- `perf`: performance improvement

Examples:
```
feat(auth): add JWT refresh token rotation
fix(session): handle expired sessions gracefully
docs(api): document connectivity.status RPC
chore(deps): upgrade tokio to 1.45
```

## Branch Naming
Pattern: `<type>/<ticket-id>-<short-description>`

Examples:
- `feat/CLAW-42-streaming-responses`
- `fix/CLAW-99-session-timeout`
- `chore/CLAW-101-update-deps`

## Branch Strategy
- `main` — always releasable; protected (require PR)
- `feat/*` — feature branches from `main`
- `fix/*` — bug fixes from `main` (or target release branch for backports)
- Never commit directly to `main`

## Pull Requests
- Small, focused PRs — one feature or fix per PR
- PR title follows Conventional Commits format
- Always write a description: what, why, how to test
- Squash merge to keep `main` history linear

## What Never Goes in Git
- Secrets, API keys, tokens
- Generated files (`node_modules/`, `target/`, `.dart_tool/`, `build/`)
- Local IDE settings (`.idea/`, `.vscode/` personal settings)
- Log files, local DB files
