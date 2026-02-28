# clawd init — Initialize a Project

`clawd init` sets up the ClawDE AFS (Agent File System) structure in your project root.

## What it creates

Running `clawd init` in a project directory creates:

```
.claw/
├── tasks/              ← task tracking (syncs with .claude/tasks/)
├── policies/
│   ├── tool-risk.json  ← per-tool risk level config
│   └── mcp-trust.json  ← MCP server trust policy
├── templates/          ← prompt templates for this project
├── evals/datasets/     ← eval datasets for automated quality checks
├── telemetry/          ← local session traces (gitignored)
├── worktrees/          ← managed git worktrees (gitignored)
├── README.md           ← structure documentation
└── .gitignore          ← ignores worktrees/ and telemetry/
```

It also seeds a `.claude/` directory with:

- `CLAUDE.md` — project instructions seeded for your detected stack
- `memory/decisions.md` — blank architecture decisions log
- `tasks/active.md` — blank sprint task file

## Usage

```bash
# Auto-detect stack from current directory
clawd init

# Initialize a specific path
clawd init /path/to/myproject

# Force a specific stack template
clawd init --template rust-cli
clawd init --template nextjs
clawd init --template react-spa
clawd init --template flutter-app
clawd init --template nself-backend
clawd init --template generic
```

## Stack auto-detection

ClawDE detects your stack by looking for marker files:

| Marker | Stack |
| --- | --- |
| `pubspec.yaml` | Flutter App |
| `package.json` + `next.config.*` | Next.js |
| `package.json` + `vite.config.*` | React SPA |
| `.env.nself` or `nself.yml` | nSelf Backend |
| `Cargo.toml` | Rust CLI |
| (none of the above) | Generic |

## Idempotency

`clawd init` is safe to run multiple times. Existing files are never overwritten — it only creates what is missing.

## Flutter app

The "Initialize Project" card appears in the ClawDE desktop app when your open project does not yet have a `.claw/` directory. Click **Initialize Project** to run the equivalent of `clawd init` without opening a terminal.

## Git

Add `.claw/worktrees/` and `.claw/telemetry/` to your `.gitignore`. `clawd init` does this automatically.

The rest of `.claw/` — tasks, policies, templates — should be committed so your team shares the same agent configuration.

## Next steps

After initializing:

1. Edit `.claude/CLAUDE.md` to add project-specific rules
2. Open ClawDE and start a session — the daemon reads `.claude/` automatically
3. Run `clawd doctor` to verify the AFS structure is complete

See also: [[Configuration]] · [[Daemon-Doctor]] · [[Folder-Structure]]
