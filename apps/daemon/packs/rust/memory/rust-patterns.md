# Rust Conventions

## Error Handling
- Use `?` operator — never `.unwrap()` or `.expect()` in production code
- Define project error type: `#[derive(Debug, thiserror::Error)]`
- Return `Result<T, Error>` from fallible functions; `Option<T>` for absence (not failure)
- `anyhow::Error` for application code; `thiserror` for library/crate boundaries

## Code Style
- Clippy clean: `cargo clippy -- -D warnings`
- Format: `cargo fmt` before every commit
- Naming: snake_case for vars/fns, CamelCase for types, SCREAMING_SNAKE_CASE for constants
- No `pub use *` re-exports (except in library `lib.rs` barrels)

## Async (Tokio)
- Use `tokio::spawn` for independent tasks; `tokio::join!` for concurrent awaits
- Wrap blocking I/O in `tokio::task::spawn_blocking`
- Avoid holding mutexes across `.await` points — use `drop(guard)` first

## Memory and Ownership
- Prefer `&str` over `String` in function parameters
- Use `Arc<Mutex<T>>` for shared mutable state across threads; `RefCell` for single-threaded
- Clone only at boundaries; prefer passing references through call chains

## Testing
- Unit tests in `#[cfg(test)]` module at the bottom of each file
- Integration tests in `tests/` directory
- Use `#[tokio::test]` for async tests
- Descriptive names: `test_session_create_returns_id_when_valid_repo`
