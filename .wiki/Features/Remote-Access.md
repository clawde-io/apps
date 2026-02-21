# Remote Access

Access your development environment from any device, anywhere. Check on sessions from your phone, review code from a browser, or connect from a second machine.

## Overview

ClawDE's remote access system lets you connect to your running daemon from outside your local network. On LAN, devices discover each other automatically. For remote access, a secure relay server handles the connection so you never need to open ports on your firewall.

## Connection Methods

### LAN Discovery (Free)

When devices are on the same network, ClawDE uses mDNS/DNS-SD (Bonjour) for automatic discovery. Your phone finds your desktop automatically — no configuration needed.

### Secure Relay (Paid — $9.99/year)

For connections outside your local network, ClawDE routes traffic through `api.clawde.io`. The relay server never sees your code — all sessions are end-to-end encrypted.

### Direct Connection (Advanced)

Power users can configure DDNS + port forwarding for direct connections without the relay. This is an advanced option for users who manage their own networking.

## Security

- **End-to-end encryption** — All remote sessions encrypted between devices
- **Device pairing** — QR code pairing establishes trust between devices
- **mTLS** — Mutual TLS authentication for relay connections
- **No code on server** — The relay forwards encrypted packets; it never decrypts your data

## How It Works

1. **Pair your device** — Scan a QR code on your desktop to pair your phone
2. **Connect** — The mobile/web app connects to your daemon (LAN or relay)
3. **Work** — Browse files, view sessions, apply diffs, run validators
4. **Disconnect** — Sessions keep running on the daemon; reconnect anytime
