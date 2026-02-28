# Coding Standards

clawd injects language and framework coding standards into every session context automatically. The agent receives a concise style guide for the detected stack before your first message — no manual "use Rust idiomatic error handling" prompts needed.

---

## How injection works

On `session.create`, the daemon:

1. Reads the `repo_path` to detect the language and framework (from file extensions and common config files).
2. Looks up the matching standards entry.
3. Appends the standards block to the session system context before any messages are exchanged.

The injected block covers naming conventions, error handling patterns, import style, and common pitfalls for that language/framework.

---

## Supported languages and frameworks

| Language / Framework | Detection signal |
| --- | --- |
| Rust | `Cargo.toml` |
| TypeScript | `tsconfig.json` or `.ts` files |
| JavaScript | `package.json` without TypeScript |
| Dart / Flutter | `pubspec.yaml` |
| Python | `pyproject.toml`, `setup.py`, or `.py` files |
| Go | `go.mod` |
| Ruby | `Gemfile` |
| Swift | `Package.swift` or `.swift` files |
| Kotlin | `build.gradle.kts` or `.kt` files |
| Java | `pom.xml` or `build.gradle` |

If no match is found, no standards block is injected.

---

## standards.list RPC

Get all available standards entries:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "standards.list",
  "params": {}
}
```

```json
{
  "standards": [
    {
      "language": "rust",
      "version": "2021",
      "summary": "Use ? for error propagation. No unwrap() in production. Clippy clean."
    },
    {
      "language": "typescript",
      "version": "5.x",
      "summary": "Strict mode. Prefer type over interface for unions. No any."
    }
  ]
}
```

---

## Example: Rust injection

When you create a session on a Rust project, the agent receives something like:

```
Language standards: Rust (2021 edition)
- Use ? operator for error propagation. No unwrap() or expect() in production code.
- Run cargo clippy --all-targets --all-features. Zero warnings allowed.
- Prefer thiserror for library errors, anyhow for application errors.
- Use tracing for structured logging. No println! in production.
- Async: tokio runtime. Avoid blocking the thread pool.
```

This is prepended once when the session opens. It counts against the context window but saves multiple correction turns later.

---

## Disabling injection

If you don't want standards injected for a session, pass `inject_standards: false` in `session.create`:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session.create",
  "params": {
    "repo_path": "/home/user/myapp",
    "provider": "claude",
    "inject_standards": false
  }
}
```

---

## See Also

- [[Features/Provider-Knowledge|Provider Knowledge]] — provider capability injection
- [[Features/Session-Manager|Session Manager]] — session lifecycle
- [[Daemon-Reference|Daemon API Reference]] — full `session.*` and `standards.*` RPC reference
- [[Home]]
