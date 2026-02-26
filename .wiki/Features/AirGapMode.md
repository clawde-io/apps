# Air-Gap Mode (Enterprise)

Air-gap mode lets ClawDE run in fully offline environments: no relay, no update checks, no external API calls. This is designed for enterprises with strict network isolation requirements.

## How It Works

When `air_gap = true` is set in `~/.claw/config.toml`:

1. The daemon skips all relay/cloud connectivity on startup
2. License verification uses an offline license bundle (Ed25519-signed) instead of the ClawDE API
3. Pack installs use a local registry path instead of `registry.clawde.io`
4. No telemetry, no update pings, no outbound connections

All AI sessions still work — they connect to your configured AI providers (Claude API, OpenAI API) directly from your machine.

## Setup

### 1. Obtain a License Bundle

Contact [enterprise@clawde.io](mailto:enterprise@clawde.io) to receive an offline license bundle file (`license.bundle`).

### 2. Install ClawDE

Use the enterprise installer (requires root):

```bash
sudo ./enterprise-install.sh \
  --license /path/to/license.bundle \
  --registry /path/to/local-packs/
```

Or install manually and configure `~/.claw/config.toml`:

```toml
[connectivity]
air_gap = true
license_path = "/etc/clawd/license.bundle"
local_registry = "/var/lib/clawd/packs"  # or http://internal-registry:8080
```

### 3. Verify

```bash
clawd license verify /etc/clawd/license.bundle
clawd license info
```

## Configuration Reference

| Setting | Type | Description |
| --- | --- | --- |
| `air_gap` | bool | Enable air-gap mode. Default: false |
| `license_path` | string | Path to offline license bundle file |
| `local_registry` | string | Local pack directory or internal registry URL |

## Status in the App

Go to **Settings → About** to see:

- Connection mode (`local` when air-gapped)
- **Air-Gap: Enabled** badge
- License validity and expiry

## What Is and Isn't Blocked

| Action | Air-Gap Behavior |
| --- | --- |
| AI sessions (Claude/Codex) | Allowed — direct to AI provider |
| Relay connections | Blocked |
| ClawDE update checks | Blocked |
| Pack install from registry.clawde.io | Blocked (use local_registry) |
| Telemetry | Blocked |
| License renewal API call | Blocked (use license bundle) |

## License Bundle Format

The bundle is a two-line file:

```
<base64url-encoded JSON payload>
<base64url-encoded Ed25519 signature>
```

The payload contains: `daemon_id`, `tier`, `seat_count`, `issued_at`, `expires_at`, `features`.

License bundles are tied to a `daemon_id` (SHA-256 of machine hardware ID) and expire at `expires_at`. Contact [enterprise@clawde.io](mailto:enterprise@clawde.io) to renew.
