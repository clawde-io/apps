#!/bin/bash
# clawd install script
# Usage: curl -sSL https://clawde.io/install | bash
# Or:    bash install.sh [--version v0.2.0] [--dir /usr/local/bin]
set -euo pipefail

REPO="clawde-io/apps"
INSTALL_DIR="${HOME}/.local/bin"
VERSION=""

# ── Argument parsing ──────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --dir)
      INSTALL_DIR="$2"
      shift 2
      ;;
    -h|--help)
      echo "Usage: install.sh [--version v0.2.0] [--dir /path/to/bin]"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

# ── Platform detection ────────────────────────────────────────────────────────

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "${OS}-${ARCH}" in
  darwin-arm64)   PLATFORM="aarch64-apple-darwin" ;;
  darwin-x86_64)  PLATFORM="x86_64-apple-darwin" ;;
  linux-x86_64)   PLATFORM="x86_64-unknown-linux-gnu" ;;
  linux-aarch64)
    echo "ERROR: Linux aarch64 is not yet distributed as a pre-built binary." >&2
    echo "       Build from source: https://github.com/clawde-io/apps" >&2
    exit 1
    ;;
  *)
    echo "ERROR: Unsupported platform: ${OS}-${ARCH}" >&2
    echo "       See https://github.com/clawde-io/apps/releases for available binaries." >&2
    exit 1
    ;;
esac

# ── Version resolution ────────────────────────────────────────────────────────

if [ -z "${VERSION}" ]; then
  echo "Fetching latest release..."
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | head -1 \
    | cut -d'"' -f4)"

  if [ -z "${VERSION}" ]; then
    echo "ERROR: Could not determine latest release. Set --version explicitly." >&2
    exit 1
  fi
fi

echo "Installing clawd ${VERSION} for ${PLATFORM}..."

# ── Download and verify ───────────────────────────────────────────────────────

BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
BINARY_URL="${BASE_URL}/clawd-${PLATFORM}"
SHA_URL="${BASE_URL}/clawd-${PLATFORM}.sha256"
TMP_BIN="$(mktemp)"
TMP_SHA="$(mktemp)"

# Clean up temp files on exit
trap 'rm -f "${TMP_BIN}" "${TMP_SHA}"' EXIT

echo "Downloading binary..."
curl -fL --progress-bar -o "${TMP_BIN}" "${BINARY_URL}"

echo "Verifying checksum..."
curl -fsSL -o "${TMP_SHA}" "${SHA_URL}"
EXPECTED_SHA="$(awk '{print $1}' "${TMP_SHA}")"

if command -v sha256sum &>/dev/null; then
  ACTUAL_SHA="$(sha256sum "${TMP_BIN}" | awk '{print $1}')"
else
  ACTUAL_SHA="$(shasum -a 256 "${TMP_BIN}" | awk '{print $1}')"
fi

if [ "${EXPECTED_SHA}" != "${ACTUAL_SHA}" ]; then
  echo "ERROR: SHA256 mismatch!" >&2
  echo "  Expected: ${EXPECTED_SHA}" >&2
  echo "  Actual:   ${ACTUAL_SHA}" >&2
  exit 1
fi

echo "Checksum verified."

# ── Install ───────────────────────────────────────────────────────────────────

mkdir -p "${INSTALL_DIR}"
install -m 0755 "${TMP_BIN}" "${INSTALL_DIR}/clawd"

echo ""
echo "clawd ${VERSION} installed to ${INSTALL_DIR}/clawd"

# ── PATH check ───────────────────────────────────────────────────────────────

if ! echo ":${PATH}:" | grep -q ":${INSTALL_DIR}:"; then
  echo ""
  echo "NOTE: ${INSTALL_DIR} is not in your PATH."
  echo "      Add it to your shell profile:"
  echo ""
  echo "      export PATH=\"${INSTALL_DIR}:\$PATH\""
  echo ""
fi

# ── Post-install hint ─────────────────────────────────────────────────────────

echo ""
echo "Next steps:"
echo "  clawd --version             # verify installation"
echo "  clawd service install       # start daemon as background service"
echo "  clawd doctor                # run system diagnostics"
echo ""
echo "Documentation: https://clawde.io/docs"
