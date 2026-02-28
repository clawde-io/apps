# Remote Access

ClawDE lets you access your development machine from any device — another laptop, phone, or tablet — over your local network or the internet.

## Connection Modes

| Mode | How | Requires |
| --- | --- | --- |
| **Local** | Desktop app on the same machine as the daemon | Nothing extra |
| **LAN** | Phone or laptop on the same Wi-Fi network | Pairing (one-time) |
| **Relay** | Phone or laptop anywhere with internet | Personal Remote plan ($9.99/yr) + Pairing |

## Pairing a Device (LAN or Relay)

Before a remote device can connect, you pair it with your host machine. Pairing is a one-time setup that takes under a minute.

### Step 1: Generate a pairing PIN on the host

**Desktop app:** Settings → Remote Access → Add Device → a 6-digit PIN appears.
**Terminal:** `clawd pair`

The PIN is valid for 10 minutes.

### Step 2: Enter the PIN on the remote device

**Mobile app:** Tap "+" → Scan QR code (the desktop shows a QR matching the PIN) → tap Connect.
**Another desktop:** Settings → Connect to Host → Enter PIN.

### Step 3: Done

The device receives a long-lived device token and stores it securely. Future connections happen automatically — no PIN needed again.

## Personal Remote ($9.99/yr)

The Personal Remote plan enables relay access so you can reach your machine from anywhere with internet, even when not on the same Wi-Fi.

After purchasing, the daemon automatically connects to `api.clawde.io/relay` using your license. Your paired devices can connect via relay with no additional setup.

Purchase at [clawde.io/#pricing](https://clawde.io/#pricing).

## Troubleshooting

**"Connection failed" on LAN:** Make sure your firewall allows port 4300. Check `clawd doctor`.

**"Relay unavailable":** Verify your Personal Remote subscription is active at `base.clawde.io`. Check internet connectivity with `clawd doctor`.

**"PIN invalid":** PINs expire after 10 minutes. Generate a new one.

**Device shows "Offline" unexpectedly:** Daemon may have restarted. Open the desktop app on the host to reconnect.

See also: [[Troubleshooting]]
