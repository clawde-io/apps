#!/usr/bin/env bash
# enterprise-install.sh — ClawDE Enterprise (air-gap) installer.
#
# Usage: sudo ./enterprise-install.sh [--license /path/to/license.bundle] [--registry /path/to/packs]
#
# Installs clawd daemon binary, systemd/launchd service, and optional offline license.

set -euo pipefail

DAEMON_VERSION="${CLAWD_VERSION:-latest}"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/clawd"
DATA_DIR="/var/lib/clawd"
SERVICE_USER="${CLAWD_USER:-clawd}"
LICENSE_PATH=""
LOCAL_REGISTRY=""
OS="$(uname -s)"

# ── Parse args ────────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case $1 in
    --license)    LICENSE_PATH="$2"; shift 2 ;;
    --registry)   LOCAL_REGISTRY="$2"; shift 2 ;;
    --version)    DAEMON_VERSION="$2"; shift 2 ;;
    --user)       SERVICE_USER="$2"; shift 2 ;;
    *) echo "Unknown argument: $1"; exit 1 ;;
  esac
done

# ── Verify root ───────────────────────────────────────────────────────────────

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Error: enterprise-install.sh must be run as root (sudo)"
  exit 1
fi

echo "=== ClawDE Enterprise Installer ==="
echo "Version: $DAEMON_VERSION"
echo "OS: $OS"
echo ""

# ── Create system user ────────────────────────────────────────────────────────

if ! id -u "$SERVICE_USER" &>/dev/null; then
  echo "Creating system user: $SERVICE_USER"
  if [[ "$OS" == "Linux" ]]; then
    useradd --system --no-create-home --shell /bin/false "$SERVICE_USER"
  elif [[ "$OS" == "Darwin" ]]; then
    # macOS: use dscl
    dscl . -create /Users/"$SERVICE_USER"
    dscl . -create /Users/"$SERVICE_USER" UserShell /usr/bin/false
    dscl . -create /Users/"$SERVICE_USER" IsHidden 1
  fi
fi

# ── Create directories ────────────────────────────────────────────────────────

mkdir -p "$CONFIG_DIR" "$DATA_DIR"
chown "$SERVICE_USER" "$DATA_DIR"

# ── Download binary ───────────────────────────────────────────────────────────

BINARY_NAME="clawd"
if [[ "$OS" == "Darwin" ]]; then
  ARCH="$(uname -m)"
  BINARY_URL="https://github.com/clawde-io/apps/releases/download/v${DAEMON_VERSION}/clawd-${ARCH}-apple-darwin"
else
  ARCH="$(uname -m)"
  BINARY_URL="https://github.com/clawde-io/apps/releases/download/v${DAEMON_VERSION}/clawd-${ARCH}-unknown-linux-gnu"
fi

echo "Downloading: $BINARY_URL"
# In air-gap: copy from local media instead
if [[ -f "./clawd" ]]; then
  echo "Using local binary: ./clawd"
  cp ./clawd "$INSTALL_DIR/$BINARY_NAME"
else
  curl -fL "$BINARY_URL" -o "$INSTALL_DIR/$BINARY_NAME"
fi
chmod +x "$INSTALL_DIR/$BINARY_NAME"
echo "Installed: $INSTALL_DIR/$BINARY_NAME"

# ── Write config ──────────────────────────────────────────────────────────────

CONFIG_FILE="$CONFIG_DIR/config.toml"
if [[ ! -f "$CONFIG_FILE" ]]; then
  cat > "$CONFIG_FILE" <<TOML
[connectivity]
air_gap = true
prefer_direct = false
${LICENSE_PATH:+license_path = "$LICENSE_PATH"}
${LOCAL_REGISTRY:+local_registry = "$LOCAL_REGISTRY"}

[observability]
log_level = "info"
TOML
  echo "Created config: $CONFIG_FILE"
fi

# ── Install license bundle ────────────────────────────────────────────────────

if [[ -n "$LICENSE_PATH" && -f "$LICENSE_PATH" ]]; then
  DEST="$CONFIG_DIR/license.bundle"
  cp "$LICENSE_PATH" "$DEST"
  chown root:root "$DEST"
  chmod 644 "$DEST"
  echo "Installed license bundle: $DEST"

  # Verify the bundle
  "$INSTALL_DIR/$BINARY_NAME" license verify "$DEST" && echo "License verified OK"
fi

# ── Install local pack registry ───────────────────────────────────────────────

if [[ -n "$LOCAL_REGISTRY" && -d "$LOCAL_REGISTRY" ]]; then
  PACK_DEST="$DATA_DIR/packs"
  mkdir -p "$PACK_DEST"
  cp -r "$LOCAL_REGISTRY"/. "$PACK_DEST/"
  chown -R "$SERVICE_USER" "$PACK_DEST"
  echo "Pack registry installed: $PACK_DEST ($(ls "$PACK_DEST" | wc -l | xargs) files)"
fi

# ── Install service ───────────────────────────────────────────────────────────

if [[ "$OS" == "Linux" ]]; then
  cat > /etc/systemd/system/clawd.service <<SERVICE
[Unit]
Description=ClawDE Daemon (Enterprise)
After=network.target

[Service]
Type=simple
User=$SERVICE_USER
ExecStart=$INSTALL_DIR/clawd serve --config $CONFIG_FILE
Restart=on-failure
RestartSec=5
Environment=CLAWD_DATA_DIR=$DATA_DIR

[Install]
WantedBy=multi-user.target
SERVICE
  systemctl daemon-reload
  systemctl enable clawd
  echo "Service installed: systemctl start clawd"

elif [[ "$OS" == "Darwin" ]]; then
  PLIST="/Library/LaunchDaemons/io.clawde.clawd.plist"
  cat > "$PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>io.clawde.clawd</string>
  <key>ProgramArguments</key>
  <array>
    <string>$INSTALL_DIR/clawd</string>
    <string>serve</string>
    <string>--config</string>
    <string>$CONFIG_FILE</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>UserName</key><string>$SERVICE_USER</string>
  <key>StandardErrorPath</key><string>/var/log/clawd.log</string>
</dict>
</plist>
PLIST
  launchctl load "$PLIST"
  echo "Service installed: launchctl start io.clawde.clawd"
fi

echo ""
echo "=== Installation complete ==="
echo "Binary: $INSTALL_DIR/$BINARY_NAME"
echo "Config: $CONFIG_FILE"
echo "Data:   $DATA_DIR"
echo ""
echo "Start the daemon:"
if [[ "$OS" == "Linux" ]]; then
  echo "  sudo systemctl start clawd"
else
  echo "  sudo launchctl start io.clawde.clawd"
fi
