# Sprint T — Wiring Notes

**Sprint:** T (Packaging & Distribution)
**Tasks covered:** DT.T01–DT.T09 (partial — CI/distribution layer only)
**Date:** 2026-02-25

---

## New Files Created

### GitHub Actions Workflows

| File | Purpose |
|------|---------|
| `.github/workflows/homebrew-tap.yml` | Fires on `release: published`. Checks out `clawde-io/homebrew-clawde` tap repo, fetches the three SHA256 files from the release assets, substitutes them into `Formula/clawd.rb` (plus version), commits, and pushes. Requires `TAP_GITHUB_TOKEN` secret with write access to the tap repo. |
| `.github/workflows/debian-package.yml` | Fires on `release: published`. Downloads the `x86_64-unknown-linux-gnu` binary, builds a proper `.deb` with `DEBIAN/control`, `DEBIAN/postinst`, `DEBIAN/prerm`, and a systemd user unit at `lib/systemd/user/clawd.service`. Uploads the `.deb` back to the same release via `softprops/action-gh-release@v2`. |
| `.github/workflows/macos-notarize.yml` | Stub only — job has `if: false`. Workflow is `workflow_dispatch`-only and fully documents all required Apple Developer secrets. Enable by removing the `if: false` condition once DT.T01/DT.T02 certs are provisioned. |

### Scripts

| File | Purpose |
|------|---------|
| `scripts/install.sh` | Standalone bash installer. Detects platform (darwin-arm64, darwin-x86_64, linux-x86_64). Fetches latest tag from GitHub API if `--version` not given. Downloads binary + SHA file, verifies checksum, installs to `~/.local/bin` (or `--dir`). Prints PATH warning if needed. |
| `scripts/clawd.rb.template` | Homebrew formula template for the `clawde-io/homebrew-clawde` tap repo. Contains `VERSION_PLACEHOLDER` and `SHA_PLACEHOLDER` markers for all three platforms (macos, macos-intel, linux). The `homebrew-tap.yml` workflow uses `sed` to substitute these on each release. |

### Wiki

| File | Purpose |
|------|---------|
| `.wiki/Packages.md` | Public-facing install guide covering all distribution channels: install script, Homebrew, .deb, Windows manual, raw binary download, verification, and uninstall. |
| `.wiki/_Sidebar.md` | Updated — added `[[Packages|Install & Distribution]]` under the Getting started section. |

---

## Wiring Required in release.yml

The existing `release.yml` does not upload `scripts/install.sh` as a release asset. Add the following step to the `release` job (after downloading artifacts, before publishing):

```yaml
      - name: Stage install script
        run: cp scripts/install.sh release-artifacts/install.sh
```

This makes `https://github.com/clawde-io/apps/releases/download/vX.Y.Z/install.sh` available, which is referenced by the `https://clawde.io/install` redirect on the marketing site.

Alternatively, the marketing site (`web/site/`) can serve `install.sh` directly as a static file and proxy to the latest release download — either approach works.

---

## Secrets Required

| Secret | Used by | Notes |
|--------|---------|-------|
| `TAP_GITHUB_TOKEN` | `homebrew-tap.yml` | Fine-grained PAT with `contents: write` on `clawde-io/homebrew-clawde`. Create at github.com/settings/tokens. |
| `APPLE_DEVELOPER_CERT_BASE64` | `macos-notarize.yml` | Exported .p12, base64-encoded. Set when Apple Developer cert is provisioned (DT.T01). |
| `APPLE_DEVELOPER_CERT_PASSWORD` | `macos-notarize.yml` | Passphrase for the .p12 export. |
| `APPLE_TEAM_ID` | `macos-notarize.yml` | 10-char Apple Developer Team ID. |
| `APPLE_NOTARY_KEY_ID` | `macos-notarize.yml` | App Store Connect API key ID. |
| `APPLE_NOTARY_KEY_ISSUER_ID` | `macos-notarize.yml` | App Store Connect API issuer UUID. |
| `APPLE_NOTARY_KEY_BASE64` | `macos-notarize.yml` | App Store Connect .p8 key file, base64-encoded. |

---

## Homebrew Tap Repo Setup

The `clawde-io/homebrew-clawde` repo must exist before the tap workflow can run. Minimum structure:

```text
homebrew-clawde/
  Formula/
    clawd.rb      ← copy of scripts/clawd.rb.template with real SHAs
```

On first run: manually create `Formula/clawd.rb` from the template with a real release's SHAs. Subsequent releases are fully automated by `homebrew-tap.yml`.

---

## Tasks Remaining (not covered by this CI layer)

| Task | Notes |
|------|-------|
| DT.T01 | macOS code signing — needs Apple Developer cert in secrets, then enable signing in `release.yml` for macOS build steps |
| DT.T02 | macOS notarization — enable `macos-notarize.yml` by removing `if: false` once DT.T01 is done |
| DT.T03 | DMG packaging — `create-dmg` step is scaffolded in `macos-notarize.yml`; wire to release upload |
| DT.T04 | Linux .deb — workflow is complete; test on Ubuntu 22.04 and 24.04 (`dpkg -i clawd.deb`) |
| DT.T05 | Linux .rpm — not yet implemented; needs `rpmbuild` step and `.spec` file added to `scripts/` |
| DT.T06 | Windows MSI — not yet implemented; needs WiX Toolset step in a Windows runner |
| DT.T07 | Windows code signing — needs Authenticode cert; add `signtool.exe` step after MSI build |
| DT.T08 | iOS TestFlight — needs `fastlane` setup in `mobile/` and App Store Connect credentials |
| DT.T09 | Android Play Store alpha — needs `google-play-action` or `fastlane supply` with service account JSON |
