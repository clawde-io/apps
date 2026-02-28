# Projects

Projects are named workspaces that group one or more git repositories under a single label. They let you switch between different codebases quickly and give the daemon context about how your work is organized.

## What is a Project?

A project has:
- A **name** (e.g., "ClawDE", "nself", "Personal")
- An optional **root path** (the parent directory, e.g., `~/Sites/clawde`)
- One or more **git repositories** (e.g., `~/Sites/clawde/apps`, `~/Sites/clawde/web`)
- An optional **org slug** (links to a GitHub org, e.g., `clawde-io`)

Projects are stored in the daemon's SQLite database and persist across restarts.

## Creating a Project

**Desktop app:** Click the project selector in the top-left → "New Project" → enter a name and optional root path.

**CLI:**
```bash
clawd project create "ClawDE" --path ~/Sites/clawde
clawd project add-repo <project-id> ~/Sites/clawde/apps
clawd project add-repo <project-id> ~/Sites/clawde/web
```

## Switching Projects

Use the project dropdown in the top-left of the desktop app. The active project determines which repos appear in the repo picker when starting a new session.

## Starting a Session in a Repo

When you start a new AI session, you select a repo from the active project (or browse to any folder). The daemon opens the repo and the AI session has full git context for that repo.

## CLI Reference

| Command | Description |
| --- | --- |
| `clawd project list` | List all projects |
| `clawd project create <name>` | Create a new project |
| `clawd project add-repo <id> <path>` | Add a git repo to a project |

## RPC Reference

See [[Daemon-Reference#project-methods]] for the full JSON-RPC API.
