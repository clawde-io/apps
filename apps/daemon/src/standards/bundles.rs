/// Embedded TOML coding standards bundles — V02.T30.
///
/// Each bundle contains top rules for a language. Injected as system prompt
/// context on session.create (V02.T31).
use super::Language;

/// Return the standards bundle text for a language.
/// Returns None for Unknown.
pub fn bundle_for(lang: &Language) -> Option<&'static str> {
    match lang {
        Language::Rust => Some(RUST_STANDARDS),
        Language::TypeScript => Some(TYPESCRIPT_STANDARDS),
        Language::Flutter => Some(FLUTTER_STANDARDS),
        Language::Python => Some(PYTHON_STANDARDS),
        Language::Go => Some(GO_STANDARDS),
        Language::Unknown => None,
    }
}

// ─── Rust ─────────────────────────────────────────────────────────────────────

const RUST_STANDARDS: &str = r#"
## Coding Standards: Rust

1. **No `unwrap()` in production code.** Use `?` operator or `.expect("reason")` with a message that explains why the value cannot be None/Err in this context. Never silently panic.
2. **Error propagation via `anyhow`.** Use `anyhow::Result<T>` for fallible functions. Add `.context("operation failed")` to add context to low-level errors before propagating.
3. **Clippy clean.** All code must pass `cargo clippy -- -D warnings` before commit. Never suppress a lint without a comment explaining why.
4. **No blocking I/O in async functions.** Use `tokio::fs`, `tokio::net`, and `tokio::task::spawn_blocking` for all I/O inside `async fn`. Never call `std::fs::read` directly in async context.
5. **Derive standard traits.** Always derive `Debug` for structs/enums. Derive `Clone` when the type needs to be shared. Derive `Serialize`/`Deserialize` for all types that cross RPC or storage boundaries.
"#;

// ─── TypeScript ───────────────────────────────────────────────────────────────

const TYPESCRIPT_STANDARDS: &str = r#"
## Coding Standards: TypeScript

1. **Strict TypeScript only.** `"strict": true` in tsconfig.json. Never use `any` — use `unknown` and narrow it, or define proper types.
2. **No `console.log` in production.** Remove all `console.log`, `console.warn`, and `console.error` before committing. Use a structured logger (pino, winston) or the project's logger module.
3. **Explicit return types on exported functions.** All exported functions and methods must have explicit return types. Inferred types are acceptable for private/internal helpers.
4. **Zod for runtime validation.** Validate all external data (API responses, user inputs, env vars) with Zod schemas. Never trust `JSON.parse()` without validation.
5. **No barrel-file re-exports for lazy loading.** Prefer direct imports over barrel files (`index.ts`) when tree-shaking matters (Next.js pages, API routes). Barrel files are fine for library packages.
"#;

// ─── Flutter / Dart ───────────────────────────────────────────────────────────

const FLUTTER_STANDARDS: &str = r#"
## Coding Standards: Flutter / Dart

1. **Immutable widgets.** Prefer `StatelessWidget` + Riverpod providers over `StatefulWidget`. Never store app state in widget fields — use providers.
2. **`const` everywhere possible.** Mark widget constructors and build methods `const` wherever widgets have no runtime state. Reduces unnecessary rebuilds.
3. **No `!` null assertion on unvalidated values.** Use `??`, `?.`, `if (x != null)`, or early returns. Only use `!` where you can prove the value is non-null at the call site.
4. **Riverpod 2.x patterns.** Use `AsyncNotifierProvider` for async data. Use `Provider` for pure derived state. Use `ref.watch` in `build`, `ref.read` in callbacks/methods only.
5. **`Color.withValues()` over `Color.withOpacity()`.** Flutter 3.27+ deprecates `withOpacity`. Always use `withValues(alpha: 0.5)` syntax.
"#;

// ─── Python ───────────────────────────────────────────────────────────────────

const PYTHON_STANDARDS: &str = r#"
## Coding Standards: Python

1. **Type hints everywhere.** All function signatures must have type hints (PEP 484). Use `from __future__ import annotations` at the top of each file for forward references.
2. **No bare `except:`.** Always catch specific exceptions: `except ValueError:`, `except (TypeError, KeyError):`. Never use `except:` or `except Exception:` without re-raising or logging.
3. **f-strings over `.format()`.** Use f-strings for string interpolation. Never use `%` formatting. `.format()` is acceptable only for template strings stored as constants.
4. **Pathlib over os.path.** Use `pathlib.Path` for all file path operations. Never use `os.path.join`, `os.path.dirname`, or string concatenation for paths.
5. **Virtual environments and pinned deps.** All dependencies must be pinned in `requirements.txt` or `pyproject.toml` with exact versions. Never install packages globally; always use a venv.
"#;

// ─── Go ───────────────────────────────────────────────────────────────────────

const GO_STANDARDS: &str = r#"
## Coding Standards: Go

1. **Always handle errors.** Never assign errors to `_` unless you have a specific documented reason. Log or propagate every non-nil error with context using `fmt.Errorf("context: %w", err)`.
2. **Explicit context propagation.** Pass `context.Context` as the first parameter to every function that does I/O, has a timeout, or can be cancelled. Never store context in a struct.
3. **Table-driven tests.** Write tests as table-driven tests using `[]struct{ name, input, want }` slices. Use `t.Run(tc.name, ...)` for subtests. Test files end with `_test.go`.
4. **Short variable names in narrow scopes.** Use single-letter names (`i`, `v`, `k`) in tight loops. Use descriptive names for function-level and package-level variables.
5. **No init() side effects.** Avoid `init()` functions with complex logic. Use explicit initialization in `main()` or constructor functions. `init()` is acceptable for registering codecs or drivers.
"#;
