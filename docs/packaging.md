# Packaging and distribution

Coqui ships the desktop app and headless CLI from one workspace. Installer budget is intentionally generous: Screaming Frog ships a several-hundred-MB installer and lands near 1 GB installed, so a Coqui installer in the **200–350 MB** range is acceptable.

## Current state

| Artifact | Status |
| --- | --- |
| Windows/macOS/Linux CLI binaries | `.github/workflows/release.yml` builds on `v*` tags and uploads `coqui` + `SHA256SUMS.txt` |
| Desktop installer | `npm run build` (Tauri) locally; release workflow not yet added |
| Node + Lighthouse | resolved from sidecar paths if present, otherwise system `npx` |
| Chromium | uses an installed Chrome/Edge; bundling pending |

## Sidecar layout

When bundling lands, the app should find these next to the executable:

```
coqui(.exe)
node(.exe)
lighthouse/lighthouse-cli/index.js
chrome/            # optional Chromium sidecar
```

Resolution order already implemented for Lighthouse:

1. `COQUI_NODE` / `COQUI_LIGHTHOUSE_CLI` environment overrides.
2. Sidecar files next to the executable.
3. System `npx` fallback.

`CHROME_PATH` is set to an installed Chrome/Edge when Lighthouse runs; a bundled Chromium path should use the same override.

## Release process

```bash
# 1. Land changes on main through feature-branch PRs.
# 2. Tag a release.
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds the CLI for the three desktop platforms, publishes the binary plus `SHA256SUMS.txt`, and fails if an expected file is missing.

## Planned slices

1. Desktop installers in the release workflow (Windows MSI/NSIS, macOS DMG, Linux AppImage/deb).
2. Bundle Node + Lighthouse with checksums; remove the system-Node requirement.
3. Bundle Chromium for rendering and Lighthouse.
4. Signing/notarization and reproducible-build notes.
5. Docker/Compose packaging for the headless server mode.
