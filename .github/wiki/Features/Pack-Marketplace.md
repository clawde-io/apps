# Pack Marketplace

Install and share bundles of AI skills, rules, agents, and validators. Build on community workflows instead of starting from scratch.

## Overview

A Pack is a versioned bundle containing any combination of skills, rules, agents, and validators. Packs let developers share their AI workflows with others. Install a security pack to get security-focused review rules. Install a React pack to get component generation skills. Compose multiple packs for a customized development experience.

## What's in a Pack?

| Component | Description |
| --- | --- |
| Skills | Reusable task bundles (e.g., "/deploy", "/test-all") |
| Rules | Behavioral constraints for AI sessions |
| Agents | Specialized AI personas for specific tasks |
| Validators | Quality gates (lint, test, typecheck, custom) |

## How It Works

### Installing Packs

```bash
# Install from the registry
clawde pack install @clawde/security
clawde pack install @clawde/react-best-practices

# Install from a local directory
clawde pack install ./my-pack

# List installed packs
clawde pack list
```

### Creating Packs

```bash
# Initialize a new pack
clawde pack init my-pack

# Structure
my-pack/
├── pack.toml          # Pack manifest (name, version, deps)
├── skills/            # Skill definitions
├── rules/             # Rule files
├── agents/            # Agent definitions
├── validators/        # Validator configs
└── README.md          # Pack documentation
```

### Publishing

```bash
# Publish to the registry
clawde pack publish
```

## Registry

The Pack Marketplace is a hosted registry at `registry.clawde.io`. Packs are versioned, signed, and searchable. Free to browse and install; publishing requires a ClawDE account.
