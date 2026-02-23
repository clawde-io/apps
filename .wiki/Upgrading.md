# Upgrading ClawDE

## Automatic Updates

ClawDE updates automatically when idle (no active sessions).

**macOS/Linux (Homebrew):** Updated via `brew upgrade clawd` or automatically via daemon self-update.

**Windows:** Updated via the Windows installer or daemon self-update.

**Mobile:** Updated via App Store or Google Play.

## Manual Update

To force an immediate update:

```bash
clawd stop
brew upgrade clawd   # macOS/Linux via Homebrew
clawd start
```

Or download the latest binary from [GitHub Releases](https://github.com/clawde-io/apps/releases).

## Database Migrations

The daemon runs SQLite migrations automatically on startup. Migrations are:
- **Additive only**: New columns, new tables — never destructive
- **Idempotent**: Safe to run multiple times
- **Backward compatible**: v0.1.0 databases upgrade cleanly to v0.2.0+

You do not need to do anything. The daemon handles migrations.

## Rollback

If you need to downgrade to a previous version:

1. Stop the daemon: `clawd stop`
2. Install the previous binary
3. Start the daemon: `clawd start`

The database is backward compatible — downgrading does not corrupt data. New columns added by the newer version are simply ignored by the older version.

## Config File Migration

If your `clawd.toml` has unrecognized keys after an upgrade, the daemon will log a warning but continue. Old keys are ignored. Check [[Configuration]] for the current key reference.

## Version History

See [[Changelog]] for version history and what changed in each release.

## Getting Help

If you encounter issues after upgrading, check [[Troubleshooting]] first, then open an issue on [GitHub](https://github.com/clawde-io/apps/issues).
