#!/usr/bin/env bash
# publish-packs.sh — Build, sign, and publish all first-party ClawDE packs.
#
# Usage: ./scripts/publish-packs.sh [--dry-run]
#
# Requires:
#   CLAWD_REGISTRY_PUBLISHER_TOKEN in env (or vault.env)
#   CLAWD_REGISTRY_URL (default: https://registry.clawde.io)

set -euo pipefail

DRY_RUN="${1:-}"
REGISTRY="${CLAWD_REGISTRY_URL:-https://registry.clawde.io}"
PACKS_DIR="$(dirname "$0")/../packs"
DIST_DIR="$(dirname "$0")/../dist/packs"

if [[ -z "${CLAWD_REGISTRY_PUBLISHER_TOKEN:-}" ]]; then
  echo "Error: CLAWD_REGISTRY_PUBLISHER_TOKEN not set"
  echo "Source ~/.claude/vault.env or set the variable"
  exit 1
fi

mkdir -p "$DIST_DIR"

PACKS=(gci react nextjs rust flutter python typescript security testing git-flow)

for pack in "${PACKS[@]}"; do
  pack_dir="$PACKS_DIR/$pack"
  if [[ ! -f "$pack_dir/pack.toml" ]]; then
    echo "WARNING: $pack_dir/pack.toml not found — skipping"
    continue
  fi

  # Read pack name from pack.toml
  pack_name=$(grep '^name' "$pack_dir/pack.toml" | head -1 | sed 's/.*= "\(.*\)"/\1/')
  pack_version=$(grep '^version' "$pack_dir/pack.toml" | head -1 | sed 's/.*= "\(.*\)"/\1/')
  tarball="$DIST_DIR/${pack}-${pack_version}.clawd-pack.tar.gz"

  echo "→ Building $pack_name v$pack_version"
  tar -czf "$tarball" -C "$pack_dir" .

  echo "  Tarball: $tarball ($(du -sh "$tarball" | cut -f1))"

  if [[ "$DRY_RUN" == "--dry-run" ]]; then
    echo "  [dry-run] Skipping publish"
    continue
  fi

  echo "  Publishing to $REGISTRY"
  response=$(curl -sf \
    -X POST \
    -H "Authorization: Bearer $CLAWD_REGISTRY_PUBLISHER_TOKEN" \
    -H "X-Pack-Name: $pack_name" \
    -H "X-Pack-Version: $pack_version" \
    -F "pack=@$tarball" \
    "$REGISTRY/v1/packs/publish" 2>&1) || {
    echo "  ERROR publishing $pack_name: $response"
    exit 1
  }
  echo "  Published: $response"
done

echo ""
echo "Done. All packs published to $REGISTRY"
