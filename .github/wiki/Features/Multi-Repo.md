# Multi-Repo Sessions

Multi-Repo Sessions let a single AI session span multiple Git repositories — useful for monorepo setups, coordinated backend/frontend changes, or cross-service refactors.

## Use cases

- Backend API change + frontend client update in one session
- Protocol buffer change propagated across multiple service repos
- Shared library bump with downstream consumer updates

## How it works

The daemon accepts a `repos` array in `session.create`. Each repo gets its own isolated context slice, but the session prompt can reference files from any of them. Tool calls (Write, Edit) specify the repo by path prefix.

```json
{
  "method": "session.create",
  "params": {
    "repos": [
      "/Users/you/sites/myapp/backend",
      "/Users/you/sites/myapp/frontend"
    ],
    "provider": "claude"
  }
}
```

## Context budget

With multiple repos, context budget is shared across all repos. ClawDE prioritizes recently modified files and files referenced in the current prompt.

## Limitations

- All repos must be on the local machine (no cross-machine multi-repo in self-hosted mode)
- Maximum 4 repos per session in v0.3.0
- Git operations (commit, push) are per-repo; there is no atomic cross-repo commit

## Status

Planned for Sprint KK.

## Related

- [Projects](Projects.md)
- [Repo Intelligence](Repo-Intelligence.md)
- [Remote Access](Remote-Access.md)
