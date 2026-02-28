#!/usr/bin/env bash
# DC.T46 — Static check for accidental secret/credential logging in the daemon source.
#
# Scans all Rust source files under src/ for patterns that could log sensitive values:
#   - debug!/info!/warn!/error!/trace! macros that reference auth_token, api_key, etc.
#   - println!/eprintln! calls with the same references
#
# Exit codes:
#   0  — no violations found
#   1  — one or more potential secret-logging violations found
#
# Usage:
#   ./scripts/check-secret-logging.sh [src-dir]
#
# Defaults to the src/ directory relative to the script location.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_DIR="${1:-$SCRIPT_DIR/../src}"

if [[ ! -d "$SRC_DIR" ]]; then
    echo "ERROR: source directory not found: $SRC_DIR" >&2
    exit 1
fi

echo "Scanning $(cd "$SRC_DIR" && pwd) for secret-logging violations..."

# Step 1: Find lines containing a log or print macro.
LOG_OR_PRINT='(debug!|info!|warn!|error!|trace!|println!|eprintln!)'

# Step 2: Among those, find lines containing a sensitive identifier.
SENSITIVE='(auth_token|api_key|api_secret|password|bearer_token|secret_key|private_key|access_token|refresh_token|client_secret)'

# Step 3: Exclude lines that are clearly safe.
SAFE='(^\s*//|redact|sanitize|REDACTED|token_path|token ready|auth token|get_or_create)'

VIOLATIONS=$(
    grep -rn -E "$LOG_OR_PRINT" "$SRC_DIR" --include="*.rs" \
        | grep -E "$SENSITIVE" \
        | grep -vE "$SAFE" \
        || true
)

echo ""
if [[ -z "$VIOLATIONS" ]]; then
    echo "OK: no secret-logging violations found."
    exit 0
else
    echo "FAIL: potential secret-logging violations found:"
    echo ""
    echo "$VIOLATIONS"
    echo ""
    echo "Review the lines above. Ensure sensitive values are not logged directly."
    echo "Use security::sanitize_tool_input() or redact the value before logging."
    exit 1
fi
