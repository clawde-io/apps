# clawd doctor — AFS Health System

The `doctor` module provides automated project health scanning for the [AFS (.claude/) directory structure](AFS-Structure). It runs as:
- A CLI command: `clawd doctor`
- A set of JSON-RPC 2.0 methods callable from any clawd client

---

## Overview

Doctor scans the `.claude/` directory of a project root and returns a **health score** (0–100) plus a list of **findings**. Each finding has a severity, a machine-readable code, a human message, and an optional file path.

The score starts at 100 and deducts penalties per severity:

| Severity | Penalty | Example |
|----------|---------|---------|
| Critical | -20 | Missing `active.md` |
| High | -10 | Missing `VISION.md` or `FEATURES.md` |
| Medium | -5 | Missing `.claude/` in `.gitignore` |
| Low | -2 | `.claude/` has >40 files |
| Info | 0 | Informational only |

Score is clamped to 0–100.

---

## RPC Reference

### `doctor.scan`

Run a health scan on a project.

**Params:**
```json
{
  "project_path": "/absolute/path/to/project",
  "scope": "all"
}
```

`scope` options: `"all"` (default), `"afs"`, `"docs"`, `"release"`

**Returns:** `DoctorScanResult`
```json
{
  "score": 85,
  "findings": [
    {
      "code": "afs.missing_vision",
      "severity": "high",
      "message": "VISION.md not found in .claude/docs/",
      "path": "/path/to/project/.claude/docs",
      "fixable": false
    }
  ]
}
```

---

### `doctor.fix`

Auto-fix one or more findings.

**Params:**
```json
{
  "project_path": "/absolute/path/to/project",
  "codes": ["afs.missing_gitignore_entry", "afs.stale_temp"]
}
```

Pass `"codes": []` to fix all fixable findings.

**Returns:** `DoctorFixResult`
```json
{
  "fixed": ["afs.missing_gitignore_entry"],
  "skipped": []
}
```

---

### `doctor.approveRelease`

Approve a release plan, setting its status to `approved`.

**Params:**
```json
{
  "project_path": "/absolute/path/to/project",
  "version": "v0.2.0"
}
```

**Returns:** `{ "ok": true }`

---

### `doctor.hookInstall`

Install the `pre-tag` git hook that blocks `git tag v*` without an approved release plan.

**Params:**
```json
{ "project_path": "/absolute/path/to/project" }
```

**Returns:** `{ "ok": true }`

---

## Findings Reference

### AFS Checks (`scope: "afs"`)

| Code | Severity | Fixable | Description |
|------|----------|---------|-------------|
| `afs.missing_claude_dir` | Critical | No | No `.claude/` directory at project root |
| `afs.missing_active_md` | Critical | No | `.claude/tasks/active.md` missing |
| `afs.missing_vision` | High | No | `.claude/docs/VISION.md` missing |
| `afs.missing_features` | High | No | `.claude/docs/FEATURES.md` missing |
| `afs.missing_pre_commit` | Medium | No | `.claude/qa/pre-commit.md` missing |
| `afs.missing_pre_pr` | Medium | No | `.claude/qa/pre-pr.md` missing |
| `afs.missing_gitignore_entry` | Medium | **Yes** | `.claude/` not in `.gitignore` |
| `afs.stale_active_md` | Low | No | `active.md` not updated in 7+ days |
| `afs.missing_ideas_dir` | Info | **Yes** | `.claude/ideas/` directory missing |
| `afs.stale_temp` | Low | **Yes** | Files in `.claude/temp/` older than 24h |

### Docs Checks (`scope: "docs"`)

| Code | Severity | Fixable | Description |
|------|----------|---------|-------------|
| `docs.both_docs_and_wiki` | High | No | Both `.docs/` and `.wiki/` present (mutual exclusivity violation) |
| `docs.brand_in_wrong_location` | Medium | No | Brand assets in `.claude/brand/` instead of `.docs/brand/` |
| `docs.too_many_claude_files` | Low | No | `.claude/` exceeds 40 files (lean files rule) |
| `docs.missing_docs_readme` | Low | **Yes** | `.docs/README.md` missing |

### Release Checks (`scope: "release"`)

| Code | Severity | Fixable | Description |
|------|----------|---------|-------------|
| `release.missing_pre_tag_hook` | Medium | **Yes** (via `doctor.hookInstall`) | `.git/hooks/pre-tag` not installed |
| `release.incomplete_plan` | Medium | No | A `release-*.md` plan missing required sections |

---

## Push Events

### `warning.versionBump`

Fires when the daemon detects a version field change in a manifest file during an active session.

**Payload:**

```json
{
  "file": "/path/to/project/Cargo.toml",
  "oldVersion": "0.1.0",
  "newVersion": "0.2.0"
}
```

**Watched files:** `Cargo.toml`, `package.json`, `pubspec.yaml` — polling every 5 seconds while a session is active in the repo.

**Purpose:** Alerts the client when a version bump happens without a FORGE-approved release plan, so the developer can run `clawd doctor` to verify the release plan is in place before tagging.

---

## CI Integration

Block merges when health score falls below 90:

```yaml
# .github/workflows/health.yml
- name: AFS health check
  run: |
    clawd doctor --scan --strict --min-score 90
    # exits non-zero if score < 90
```

_(Note: `--strict` and `--min-score` CLI flags are planned for a future sprint.)_

---

## clawd init — Stack-Detected Templates

`clawd init` auto-detects the project stack and scaffolds matching templates:

| Stack | Detected by | CLAUDE.md |
|-------|-------------|-----------|
| `rust-cli` | `Cargo.toml` present | Rust/Tokio rules |
| `nextjs` | `package.json` + `next.config.*` | Next.js App Router rules |
| `react-spa` | `package.json` + `vite.config.*` | Vite + React rules |
| `flutter-app` | `pubspec.yaml` | Flutter + Riverpod rules |
| `nself-backend` | `.env.nself` or `nself.yml` | nSelf CLI rules |
| `generic` | fallback | Blank template |

Override detection with `--template`:
```bash
clawd init ~/Sites/my-project --template nextjs
```

---

## See Also

- [AFS Structure](AFS-Structure)
- [Worktrees](Tasks/Worktrees)
- [Home](Home)
