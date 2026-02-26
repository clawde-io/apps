# Flutter/Dart Conventions

## Riverpod Patterns
- Use `@riverpod` codegen — avoid manual provider declarations for new code
- `AsyncNotifier` for async state with loading/error/data
- `Notifier` for synchronous state
- Never use `ref.read` in build methods — use `ref.watch`
- Keep providers small and focused; compose with `ref.watch`

## Widget Rules
- Prefer `const` constructors everywhere possible
- Use `ConsumerWidget` (not `StatefulWidget`) when state is from Riverpod
- Only use `StatefulWidget` for local ephemeral UI state (animation controllers, text controllers)
- `Color.withValues()` not `Color.withOpacity()` (Flutter 3.27+)

## File Structure
```
lib/
  features/         # Feature modules
    {feature}/
      {feature}_screen.dart
      widgets/
      providers/
  widgets/           # Shared widgets
  theme/             # App theme
```

## Error Handling in Dart
- Use `Result<T, E>` pattern via `sealed class` for domain errors
- Prefer `?` null-safety over null checks with `!`
- `try/catch` only at async boundaries; propagate errors upward

## Testing
- Widget tests: `pumpWidget`, `find.byType`, `tester.tap`
- Use `ProviderContainer` + `ProviderScope` for provider tests
- Mock with `mockito` or `mocktail` — never test with real network calls
