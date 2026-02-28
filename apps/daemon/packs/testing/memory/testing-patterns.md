# Testing Conventions

## Test Naming
Pattern: `test_{unit}_{condition}_{expected}`

Examples:
- `test_create_session_with_valid_repo_returns_session_id`
- `test_create_session_with_missing_repo_returns_not_found_error`
- `test_authenticate_with_expired_token_returns_unauthorized`

## AAA Pattern
Every test: Arrange → Act → Assert
```python
def test_calculate_total_with_discount_applies_percentage():
    # Arrange
    cart = Cart(items=[Item(price=100)])
    discount = Discount(pct=10)
    # Act
    total = calculate_total(cart, discount)
    # Assert
    assert total == 90
```

## What to Test
- Public interfaces only — never private methods
- Edge cases: empty input, max bounds, invalid types, concurrent access
- Error paths: every `Result::Err` or exception variant
- State transitions: what changes when an operation succeeds/fails

## What NOT to Test
- Implementation details (variable names, internal state structure)
- Framework behavior (don't test that `useState` works)
- Third-party libraries (test your integration with them, not their internals)

## Coverage
- Target: 80%+ line coverage; 70%+ branch coverage
- 100% coverage ≠ good tests — measure mutation kill rate instead
- Uncovered code is a risk signal, not an immediate bug

## Test Doubles
- Stub: returns fixed data
- Mock: verifies interactions (use sparingly)
- Fake: working implementation (e.g. in-memory DB)
- Prefer Fakes over Mocks for complex dependencies
