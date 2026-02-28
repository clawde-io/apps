# Connectivity

ClawDE connects your client apps (desktop, mobile, web) to the `clawd` daemon running on your machine. Three transport modes are supported.

## Transport modes

| Mode | When used | Latency |
| --- | --- | --- |
| **Local** | Client and daemon on the same machine | < 1ms |
| **Direct** | LAN/mDNS peer discovered on same network | 1–10ms |
| **VPN** | Explicit IP configured, no relay hop | 1–20ms |
| **Relay** | Off-LAN, internet connection via `api.clawde.io` | 20–300ms |

The **current mode** is shown in the status bar. Tap the RTT dot to see details.

## Relay (default)

The relay at `wss://api.clawde.io/relay/ws` forwards encrypted frames between the client and your daemon. End-to-end encryption (X25519 + ChaCha20-Poly1305) ensures Anthropic cannot read your messages.

Required: Personal Remote tier ($9.99/yr) or any Cloud tier.

## Direct LAN (mDNS)

When your client and daemon are on the same local network, the client can skip the relay entirely:

1. The daemon advertises `_clawde._tcp.local.` via mDNS/DNS-SD
2. The client browses for peers and connects directly via WebSocket
3. No relay server involved — lowest latency, works offline

Enable via **Settings → Connectivity → Prefer Direct** or set `prefer_direct = true` in `config.toml`.

Peer discovery is visible in **Settings → Connectivity → LAN Peer Discovery**.

## VPN / enterprise

For enterprise setups where the daemon runs on a VPN IP:

1. Enter the VPN host in **Settings → Connectivity → VPN / Enterprise IP**
2. The client connects directly — no relay, no mDNS needed

See [VPN Mode](VPNMode.md) for full setup guide.

## Connection quality indicator

The **RTT dot** in the bottom status bar shows live connection health:

| Color | Meaning |
| --- | --- |
| Green | < 50ms — excellent |
| Amber | 50–150ms — good |
| Red | ≥ 150ms or packet loss — degraded |

Tap the dot to see the full connectivity detail sheet (mode, RTT, packet loss, LAN peers).

A `connectivity_degraded` push event fires when RTT exceeds 500ms or packet loss exceeds 5%. The UI shows a warning toast.

## Daemon-side config

```toml
# ~/.config/clawd/config.toml (Linux) or ~/Library/Application Support/clawd/config.toml (macOS)

[connectivity]
prefer_direct = false   # try LAN before relay (2s timeout)
vpn_host = ""           # explicit VPN IP — leave blank for relay
air_gap = false         # disable all outbound calls (enterprise)
```

## `connectivity.status` RPC

```json
{
  "method": "connectivity.status",
  "params": {}
}
```

Response:

```json
{
  "mode": "relay",
  "rtt_ms": 42,
  "packet_loss_pct": 0.0,
  "degraded": false,
  "last_ping_at": 1709000000,
  "prefer_direct": false,
  "vpn_host": null,
  "air_gap": false,
  "lan_peers": [
    {
      "name": "clawd-abc12345._clawde._tcp.local.",
      "address": "192.168.1.10",
      "port": 4300,
      "version": "0.2.0",
      "daemon_id": "abc12345...",
      "last_seen": 1709000000
    }
  ]
}
```

## Push events

| Event | When fired |
| --- | --- |
| `connectivity_degraded` | RTT > 500ms or packet loss > 5% |
| `connectivity_restored` | Quality returns to normal after degradation |

## See also

- [VPN Mode](VPNMode.md) — enterprise LAN / VPN setup
- [Background Mode](BackgroundMode.md) — keeping the daemon running when the window is closed
- [CLI Reference](../CLI-Reference.md)
