# Official Packs

ClawDE ships 10 official packs maintained by the ClawDE team. Each pack installs memory files, automations, evals, and workflow recipes tailored for a specific language or workflow.

## Install a Pack

```bash
clawd pack install @clawde/rust
```

All official packs are available at [registry.clawde.io](https://registry.clawde.io).

## Available Packs

### @clawde/gci

Global Claude Instructions (GCI) framework. Installs memory templates for preferences, identity, and session protocol. For teams using the three-tier GCI instruction system.

```bash
clawd pack install @clawde/gci
```

### @clawde/react

React conventions, component naming, hook patterns, and test patterns. Automation: run Jest after each AI session.

```bash
clawd pack install @clawde/react
```

### @clawde/nextjs

Next.js App Router patterns, server action conventions, ISR patterns. Automation: run `pnpm build` after each session.

```bash
clawd pack install @clawde/nextjs
```

### @clawde/rust

Rust conventions, clippy rules, `?` operator error handling, and cargo test automation. Automation: run `cargo test` and `cargo clippy` after each session.

```bash
clawd pack install @clawde/rust
```

### @clawde/flutter

Flutter/Dart conventions, Riverpod patterns, widget testing rules. Automation: run `flutter test` and `flutter analyze` after each session.

```bash
clawd pack install @clawde/flutter
```

### @clawde/python

Python type hints, pytest patterns, ruff lint rules. Automation: run `pytest` and `mypy` after each session.

```bash
clawd pack install @clawde/python
```

### @clawde/typescript

TypeScript strict mode, zod schema patterns, Prettier config. Automation: run `tsc --noEmit` after each session.

```bash
clawd pack install @clawde/typescript
```

### @clawde/security

OWASP Top 10 eval cases, npm audit automation, secret scanning on file save.

```bash
clawd pack install @clawde/security
```

### @clawde/testing

TDD workflow recipe, coverage threshold automation, test naming conventions, and test generation evals.

```bash
clawd pack install @clawde/testing
```

### @clawde/git-flow

Conventional Commits format, feature-to-PR workflow recipe, branch naming conventions, commit message linting.

```bash
clawd pack install @clawde/git-flow
```

## What a Pack Contains

Each pack may include any combination of:

| Component | Description |
| --- | --- |
| `memory/` | Conventions and patterns loaded into AI context |
| `automations/` | Triggers that run commands after sessions or on file save |
| `evals/` | Test cases for evaluating AI code generation quality |
| `workflows/` | Multi-step recipes for common development workflows |

## Building Your Own Pack

See the [Pack Format reference](PackFormat.md) and publish to the registry with:

```bash
clawd pack publish ./my-pack/
```
