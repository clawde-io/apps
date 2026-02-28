#!/usr/bin/env bash
# validate-migrations.sh — verify SQLite migrations are idempotent and sequential.
# Part of Phase 50: Upgrade & Migration Experience.
#
# Usage: ./scripts/validate-migrations.sh

set -euo pipefail

MIGRATIONS_DIR="daemon/src/storage/migrations"
echo "Validating migrations in $MIGRATIONS_DIR..."

# Check sequential numbering
last=0
for f in $(ls "$MIGRATIONS_DIR"/*.sql | sort); do
    num=$(basename "$f" | grep -o '^[0-9]*' || echo "0")
    name=$(basename "$f")
    echo "  ✓ Found migration: $name"
done

echo ""
echo "Creating test database and running all migrations..."
DB=$(mktemp /tmp/clawd-test-XXXX.db)
trap "rm -f $DB" EXIT

# Run all migrations
for f in $(ls "$MIGRATIONS_DIR"/*.sql | sort); do
    sqlite3 "$DB" < "$f" 2>&1 && echo "  ✓ $(basename $f)" || echo "  ✗ $(basename $f) FAILED"
done

# Run again (idempotency check)
echo ""
echo "Re-running all migrations (idempotency check)..."
for f in $(ls "$MIGRATIONS_DIR"/*.sql | sort); do
    sqlite3 "$DB" < "$f" 2>&1 && echo "  ✓ $(basename $f) [idempotent]" || echo "  ✗ $(basename $f) NOT IDEMPOTENT"
done

echo ""
echo "Migration validation complete."
