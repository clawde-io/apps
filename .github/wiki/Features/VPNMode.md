# VPN Mode

Connect your ClawDE client directly to a `clawd` daemon running on a VPN or LAN IP — without the relay server.

## When to use VPN Mode

- Your daemon is on a corporate VPN (e.g. `10.0.1.5`)
- You are on the same LAN but the machine is not discoverable via mDNS
- You want zero relay hops for the lowest latency
- Air-gap enterprise environment with no outbound internet access

## Configuration

### Via Settings UI

1. Open **Settings → Connectivity**
2. Enter the daemon IP or hostname in the **VPN / Enterprise IP** field
3. Click **Save** — the app reconnects immediately

### Via `config.toml`

```toml
[connectivity]
# Connect to daemon at this IP instead of relay (no relay hop)
vpn_host = "10.0.1.5"

# Optionally prefer direct mDNS discovery before relay fallback
prefer_direct = true

# Enterprise air-gap: disable all outbound relay/API calls
air_gap = false
```

The config file is at:

| Platform | Path |
| --- | --- |
| macOS | `~/Library/Application Support/clawd/config.toml` |
| Linux | `~/.local/share/clawd/config.toml` |
| Windows | `%APPDATA%\clawd\config.toml` |

Restart `clawd` after editing `config.toml`.

## Connection priority

When `vpn_host` is set:

1. Client connects directly to `ws://{vpn_host}:4300`
2. Standard auth handshake (`daemon.auth`) — your auth token is still required
3. If the direct connection fails, the client falls back to relay (unless `air_gap = true`)

When `prefer_direct = true` (no `vpn_host`):

1. Client tries mDNS-discovered peer first (2s timeout)
2. Falls back to relay if no direct peer responds

## Latency

| Mode | Typical RTT |
| --- | --- |
| Local (localhost) | < 1ms |
| VPN / LAN direct | 1–10ms |
| Relay (same continent) | 20–80ms |
| Relay (cross-continent) | 100–300ms |

The **Connection Quality indicator** in the status bar shows live RTT and fires a `connectivity_degraded` event when RTT exceeds 500ms or packet loss exceeds 5%.

## Air-gap mode

Set `air_gap = true` to disable all outbound calls:

- No relay connection
- No license verification (Free tier only — no license needed)
- No auto-update checks
- No telemetry

The daemon accepts local WebSocket connections only. Clients must connect via explicit IP or mDNS.

## Security

Direct and VPN connections use the same `daemon.auth` token as relay connections. The auth token is stored in:

- macOS/Linux: `~/.claw/auth.token`
- Windows: `%APPDATA%\.claw\auth.token`

Never share this token. Rotate it with `clawd token rotate`.

## See also

- [Connectivity](Connectivity.md) — relay vs direct vs VPN overview
- [Background Mode](BackgroundMode.md) — keeping the daemon running
- [CLI Reference](../CLI-Reference.md)
