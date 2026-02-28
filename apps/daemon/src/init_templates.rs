/// Stack detection and built-in template definitions for `clawd init` (D64.T19-T22).
///
/// `detect_stack(path)` inspects the project root for well-known marker files
/// and returns one of five stack identifiers. Template content is embedded in
/// the binary — no network access required.

// ─── Stack identifier ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stack {
    RustCli,
    NextJs,
    ReactSpa,
    FlutterApp,
    NselfBackend,
    /// Fallback when no stack-specific markers are found.
    Generic,
}

impl Stack {
    pub fn as_str(self) -> &'static str {
        match self {
            Stack::RustCli => "rust-cli",
            Stack::NextJs => "nextjs",
            Stack::ReactSpa => "react-spa",
            Stack::FlutterApp => "flutter-app",
            Stack::NselfBackend => "nself-backend",
            Stack::Generic => "generic",
        }
    }
}

impl std::fmt::Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Stack {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "rust-cli" | "rust" => Ok(Stack::RustCli),
            "nextjs" | "next" => Ok(Stack::NextJs),
            "react-spa" | "react" | "vite" => Ok(Stack::ReactSpa),
            "flutter" | "flutter-app" => Ok(Stack::FlutterApp),
            "nself" | "nself-backend" => Ok(Stack::NselfBackend),
            "generic" => Ok(Stack::Generic),
            _ => Err(()),
        }
    }
}

// ─── Stack detection ──────────────────────────────────────────────────────────

/// Detect the project stack by inspecting marker files in `project_root`.
///
/// Detection order (most specific first):
///   1. `pubspec.yaml`           → Flutter app
///   2. `package.json` + `next.config.*`  → Next.js
///   3. `package.json` + `vite.config.*`  → React SPA
///   4. `.env.nself` or both `docker-compose.yml`+`nself.yml` → nSelf backend
///   5. `Cargo.toml`             → Rust CLI
///   6. fallback                 → Generic
pub fn detect_stack(project_root: &std::path::Path) -> Stack {
    // Flutter: pubspec.yaml is definitive
    if project_root.join("pubspec.yaml").exists() {
        return Stack::FlutterApp;
    }

    // Next.js: package.json + next.config.{js,ts,mjs,cjs}
    if project_root.join("package.json").exists() {
        let has_next = [
            "next.config.js",
            "next.config.ts",
            "next.config.mjs",
            "next.config.cjs",
        ]
        .iter()
        .any(|f| project_root.join(f).exists());
        if has_next {
            return Stack::NextJs;
        }

        // Vite/React SPA: package.json + vite.config.*
        let has_vite = ["vite.config.js", "vite.config.ts", "vite.config.mjs"]
            .iter()
            .any(|f| project_root.join(f).exists());
        if has_vite {
            return Stack::ReactSpa;
        }
    }

    // nSelf backend: .env.nself marker or nself.yml
    if project_root.join(".env.nself").exists() || project_root.join("nself.yml").exists() {
        return Stack::NselfBackend;
    }

    // Rust CLI: Cargo.toml
    if project_root.join("Cargo.toml").exists() {
        return Stack::RustCli;
    }

    Stack::Generic
}

// ─── Template data ────────────────────────────────────────────────────────────

pub struct TemplateFiles {
    pub claude_md: &'static str,
    pub decisions_md: &'static str,
    pub gitignore_additions: &'static str,
}

pub fn template_for(stack: Stack) -> TemplateFiles {
    match stack {
        Stack::RustCli => TemplateFiles {
            claude_md: RUST_CLI_CLAUDE_MD,
            decisions_md: RUST_CLI_DECISIONS_MD,
            gitignore_additions: RUST_CLI_GITIGNORE,
        },
        Stack::NextJs => TemplateFiles {
            claude_md: NEXTJS_CLAUDE_MD,
            decisions_md: NEXTJS_DECISIONS_MD,
            gitignore_additions: NEXTJS_GITIGNORE,
        },
        Stack::ReactSpa => TemplateFiles {
            claude_md: REACT_SPA_CLAUDE_MD,
            decisions_md: REACT_SPA_DECISIONS_MD,
            gitignore_additions: REACT_SPA_GITIGNORE,
        },
        Stack::FlutterApp => TemplateFiles {
            claude_md: FLUTTER_APP_CLAUDE_MD,
            decisions_md: FLUTTER_APP_DECISIONS_MD,
            gitignore_additions: FLUTTER_APP_GITIGNORE,
        },
        Stack::NselfBackend => TemplateFiles {
            claude_md: NSELF_BACKEND_CLAUDE_MD,
            decisions_md: NSELF_BACKEND_DECISIONS_MD,
            gitignore_additions: NSELF_BACKEND_GITIGNORE,
        },
        Stack::Generic => TemplateFiles {
            claude_md: GENERIC_CLAUDE_MD,
            decisions_md: GENERIC_DECISIONS_MD,
            gitignore_additions: GENERIC_GITIGNORE,
        },
    }
}

// ─── Rust CLI template ────────────────────────────────────────────────────────

const RUST_CLI_CLAUDE_MD: &str = r#"# Project Instructions — Rust CLI

> Read the GCI at `~/.claude/CLAUDE.md` for global protocols.

## Stack

- Language: Rust (edition 2021)
- CLI: `clap` (derive API)
- Async: `tokio` (only if async is required)
- Error handling: `anyhow` for binaries, `thiserror` for libraries
- Testing: `cargo test` + `cargo nextest`

## Hard Rules

- No `unwrap()` or `expect()` in production code — use `?` and `anyhow::bail!`
- Clippy must pass: `cargo clippy -- -D warnings`
- All public functions must have doc comments
- Never use `std::process::exit` except in `main` after a fatal error
- Keep `Cargo.toml` minimal — no unused deps
"#;

const RUST_CLI_DECISIONS_MD: &str = r#"# Architecture Decisions — Rust CLI

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Rust | Performance + memory safety |
| CLI framework | clap (derive) | Ergonomic, well-maintained |
| Error handling | anyhow (bin) + thiserror (lib) | Idiomatic Rust error propagation |
| Async runtime | tokio | Standard for async Rust |
"#;

const RUST_CLI_GITIGNORE: &str = r#"
# Rust
/target/
Cargo.lock.bak
"#;

// ─── Next.js template ─────────────────────────────────────────────────────────

const NEXTJS_CLAUDE_MD: &str = r#"# Project Instructions — Next.js

> Read the GCI at `~/.claude/CLAUDE.md` for global protocols.

## Stack

- Framework: Next.js (App Router)
- Language: TypeScript (strict mode)
- Styling: Tailwind CSS
- Package manager: pnpm (never npm or yarn)
- Deployment: Vercel

## Hard Rules

- `pnpm` only — never `npm install` or `yarn`
- TypeScript strict mode — no `any` without a comment explaining why
- No `console.log` in production code
- All pages must be server components by default; add `"use client"` only when needed
- Environment variables go in `.env.local` (gitignored) — never hardcode
"#;

const NEXTJS_DECISIONS_MD: &str = r#"# Architecture Decisions — Next.js

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Framework | Next.js (App Router) | SSR/SSG, file-based routing |
| Language | TypeScript strict | Type safety |
| Styling | Tailwind CSS | Utility-first, no CSS files needed |
| Package manager | pnpm | Disk efficient, faster than npm |
| Deployment | Vercel | Native Next.js support |
"#;

const NEXTJS_GITIGNORE: &str = r#"
# Next.js
.next/
out/
.vercel
"#;

// ─── React SPA template ───────────────────────────────────────────────────────

const REACT_SPA_CLAUDE_MD: &str = r#"# Project Instructions — React SPA

> Read the GCI at `~/.claude/CLAUDE.md` for global protocols.

## Stack

- Build tool: Vite
- Framework: React 18+
- Language: TypeScript (strict mode)
- Styling: Tailwind CSS
- State: Zustand or React Query (check `package.json`)
- Package manager: pnpm (never npm or yarn)

## Hard Rules

- `pnpm` only — never `npm install` or `yarn`
- TypeScript strict mode — no `any` without a comment
- No `console.log` in production code — use a proper logger
- All data fetching goes through React Query — no raw `fetch` in components
- Environment variables via `import.meta.env.VITE_*`
"#;

const REACT_SPA_DECISIONS_MD: &str = r#"# Architecture Decisions — React SPA

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Build tool | Vite | Fast HMR, ESM native |
| Framework | React 18 | Component model, ecosystem |
| Language | TypeScript strict | Type safety |
| Styling | Tailwind CSS | Utility-first, consistent |
| Package manager | pnpm | Disk efficient |
"#;

const REACT_SPA_GITIGNORE: &str = r#"
# Vite / React
dist/
.vite/
"#;

// ─── Flutter app template ─────────────────────────────────────────────────────

const FLUTTER_APP_CLAUDE_MD: &str = r#"# Project Instructions — Flutter App

> Read the GCI at `~/.claude/CLAUDE.md` for global protocols.

## Stack

- Framework: Flutter (latest stable)
- Language: Dart
- State: Riverpod 2.x
- Navigation: go_router
- Package manager: Flutter pub

## Hard Rules

- Minimum Flutter 3.27.0 (Color.withValues requires Dart 3.7)
- Riverpod for all state — no StatefulWidget for business logic
- No `print()` in production — use a logging package
- Run `flutter analyze` before any commit — zero issues required
- Never commit generated files (`*.g.dart`, `*.freezed.dart`) directly
"#;

const FLUTTER_APP_DECISIONS_MD: &str = r#"# Architecture Decisions — Flutter App

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Framework | Flutter | Cross-platform, single codebase |
| State management | Riverpod 2.x | Compile-safe providers |
| Navigation | go_router | Deep-link support, declarative |
| Min Flutter version | 3.27.0 | Color.withValues API |
"#;

const FLUTTER_APP_GITIGNORE: &str = r#"
# Flutter
.dart_tool/
.flutter-plugins
.flutter-plugins-dependencies
build/
*.g.dart
*.freezed.dart
"#;

// ─── nSelf backend template ───────────────────────────────────────────────────

const NSELF_BACKEND_CLAUDE_MD: &str = r#"# Project Instructions — nSelf Backend

> Read the GCI at `~/.claude/CLAUDE.md` for global protocols.

## Stack

- Runtime: nSelf CLI (Postgres + Hasura + Auth)
- Language: TypeScript (Node.js services) or SQL (migrations)
- Package manager: pnpm (for any Node.js services)

## Hard Rules

- All backend management via `nself` CLI — NEVER raw Docker commands
- Database migrations go through `nself db migrate up`
- Never commit `.env` files — use `.env.example` for documentation
- All secrets in `~/.claude/vault.env`

## nSelf CLI Reference

```bash
nself build    # regenerate docker-compose from .env
nself start    # start all services
nself stop     # stop services
nself status   # check service health
nself logs     # view logs
nself db migrate up   # run migrations
```
"#;

const NSELF_BACKEND_DECISIONS_MD: &str = r#"# Architecture Decisions — nSelf Backend

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Runtime | nSelf CLI | Managed Postgres + Hasura + Auth |
| DB | PostgreSQL | Relational, Hasura support |
| API | Hasura GraphQL | Auto-generated from schema |
| Auth | Hasura Auth | JWT + social providers |
"#;

const NSELF_BACKEND_GITIGNORE: &str = r#"
# nSelf / Docker
docker-compose.override.yml
.env
.env.local
postgres-data/
"#;

// ─── Generic template ─────────────────────────────────────────────────────────

const GENERIC_CLAUDE_MD: &str = r#"# Project Instructions

> Read the GCI at `~/.claude/CLAUDE.md` for global protocols.

## Project Overview

[Describe this project here]

## Hard Rules

[Add project-specific rules here]
"#;

const GENERIC_DECISIONS_MD: &str = r#"# Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
"#;

const GENERIC_GITIGNORE: &str = "";
