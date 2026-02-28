# Python Conventions

## Type Annotations
- All public functions/methods have type annotations: `def get_user(user_id: int) -> User | None:`
- Use `from __future__ import annotations` for forward references
- Prefer `X | None` over `Optional[X]` (Python 3.10+)
- Use `TypeAlias` for complex types: `UserId: TypeAlias = int`

## Code Style
- Lint: `ruff check .` — treats warnings as errors in CI
- Format: `ruff format .`
- Line length: 100 (configured in `pyproject.toml`)
- `f-strings` over `.format()` or `%` formatting

## Error Handling
- Specific exceptions over broad `except Exception`
- Custom exceptions inherit from domain base: `class AppError(Exception): pass`
- Context managers (`with`) for resource cleanup — never bare `try/finally` for file close
- Never `pass` in except blocks — at minimum log the error

## Testing (pytest)
- Test files: `test_{module}.py` or `{module}_test.py`
- Test functions: `test_{what}_{condition}_{expected_outcome}`
- Use `pytest.fixture` for shared setup; parametrize with `@pytest.mark.parametrize`
- Mock with `pytest-mock` (`mocker.patch`); avoid `unittest.mock` directly
- `conftest.py` for shared fixtures across test files

## Project Structure
```
src/{package}/    # Main package (src layout)
tests/            # Test files mirror src structure
pyproject.toml    # Dependencies, ruff/mypy config
```
