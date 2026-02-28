# Workflow Recipes

Workflows let you chain multiple AI steps together into a reusable recipe. Run a recipe with one click from the desktop app or a single CLI command.

## What workflows do

A workflow is a sequence of AI prompts that run in order. Each step can inherit the context from the previous one, so you can build multi-stage pipelines without copy-pasting output between sessions.

Built-in recipes ship with ClawDE. You can also create your own in YAML.

## Built-in recipes

| Recipe | Steps | What it does |
| ------ | ----- | ------------ |
| `code-review` | 3 | Summarize changes, check for bugs, write a review comment |
| `release-prep` | 3 | Generate a changelog entry, bump version, draft release notes |
| `onboard-codebase` | 2 | Map the architecture, summarize key files |
| `debug-session` | 2 | Reproduce the bug, propose a fix |
| `spec-to-impl` | 2 | Convert a spec file into a task list, then implement |

## Running a workflow

From the desktop app: open the Workflows screen and tap **Run** on any recipe.

From the CLI:

```sh
clawd recipe list
clawd recipe run code-review
clawd recipe run code-review --repo /path/to/project
```

The run starts immediately and executes steps in the background. Progress appears in the desktop app as each step completes.

## Creating a workflow

From the desktop app: tap **New Workflow** to open the YAML editor.

From the CLI, write a YAML file and import it:

```sh
clawd recipe import my-workflow.yaml
```

### YAML format

```yaml
steps:
  - prompt: "Summarize the changes in the last commit."
    provider: claude
  - prompt: "Based on the summary, suggest any improvements."
    inherit_from: previous
```

Fields:

| Field | Required | Description |
| ----- | -------- | ----------- |
| `prompt` | yes | The instruction to send to the AI |
| `provider` | no | `claude` (default), `codex`, or any configured provider |
| `inherit_from` | no | `previous` — passes the previous step's output as context |

## RPC reference

| Method | Description |
| ------ | ----------- |
| `workflow.list` | List all recipes |
| `workflow.create` | Create a recipe from YAML |
| `workflow.run` | Start a recipe run (async) |
| `workflow.delete` | Delete a user-defined recipe |

Push events emitted during a run:

| Event | When |
| ----- | ---- |
| `workflow.stepCompleted` | Each step finishes |
| `workflow.ran` | All steps complete (or a step fails) |

## CLI reference

```
clawd recipe list
clawd recipe run <recipe-id> [--repo <path>]
clawd recipe import <file.yaml>
```

See [CLI Reference](../CLI-Reference.md) for full options.
