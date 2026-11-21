# Packaging and distribution

Coqui ships the desktop app and headless CLI from one workspace. The installer budget is intentionally generous: Screaming Frog ships a several-hundred-MB installer and lands near 1 GB installed, so a Coqui installer in the **200–350 MB** range is acceptable.

## Current state

| Artifact | Status |
| --- | --- |
| CLI binaries | `.github/workflows/release.yml` builds on `v*` tags for Linux/macOS/Windows and uploads OS/arch-qualified assets (`coqui-linux-x86_64`, `coqui-macos-aarch64`, `coqui-windows-x86_64.exe`) with `SHA256SUMS-<os>-<arch>.txt` |
| Desktop installers | Release jobs build Windows NSIS/MSI, macOS DMG and Linux AppImage/DEB with fetched sidecars, assert a 350 MB installer budget and upload per-platform checksums |
| Node + Lighthouse | Bundled through `npm run sidecars` in the desktop jobs; runtime resolution falls back to system `npx` |
| Chromium | Uses an installed Chrome/Edge or a Chrome for Testing build installed by `coqui browser install`; not bundled |
| Docker | CLI/API image plus `docker-compose.yml` (loopback port, data volume) |
| Signing | Windows/macOS steps are wired behind secrets and no-op without certificates; artifacts are unsigned today |
| Published release | **None yet** — no tags exist, so the workflow has not produced release assets |

## Fetching sidecars

Sidecars are bundled via `bundle.resources` (`sidecars/**/*`); at runtime the app resolves them from the Tauri resource directory, falling back to next to the executable. A tracked `src-tauri/sidecars/README.md` keeps the glob valid when sidecars have not been fetched.

`npm run sidecars` downloads a pinned Node build (checksum verified against `SHASUMS256.txt`) and installs Lighthouse with production dependencies into `src-tauri/sidecars/` (ignored by git). Verified: Node v22.14.0 and Lighthouse 12.8.2 (201 packages) run from the sidecar (`node .../cli/index.js --version` → 12.8.2).

## Sidecar layout

```
coqui(.exe)
node(.exe)
lighthouse/lighthouse-cli/index.js
```

Resolution order for Lighthouse:

1. `COQUI_NODE` / `COQUI_LIGHTHOUSE_CLI` environment overrides.
2. Sidecar files next to the executable or in the resource directory.
3. System `npx` fallback.

`CHROME_PATH` is set to a discovered Chrome/Edge when Lighthouse runs; a bundled Chromium path would use the same override.

## Release process

```bash
# 1. Land changes on main through feature-branch PRs.
# 2. Update CHANGELOG.md and bump the workspace version.
# 3. Tag and push.
git tag v0.1.0
git push origin v0.1.0
```

The release workflow:

- builds the CLI on Ubuntu, macOS and Windows and packages `dist/coqui-<os>-<arch>[.exe]` plus `dist/SHA256SUMS-<os>-<arch>.txt`;
- builds desktop installers with sidecars and per-platform checksum files;
- runs signing when the relevant secrets exist;
- fails when an expected artifact is missing or an installer exceeds 350 MB;
- attaches everything to the GitHub release.

Requirements and the versioning policy are in [release.md](release.md) and [versioning.md](versioning.md). Signing details are in [signing.md](signing.md).

## Remaining gaps

1. **First tagged release.** No release exists, so download and update docs cannot be exercised end to end.
2. **Architecture coverage.** The release matrix builds on the native runner only (Linux x86_64, macOS arm64, Windows x86_64); Linux arm64 and macOS x86_64 are not produced.
3. **Bundled Chromium.** Rendering depends on a discovered browser or the Chrome for Testing installer; decide whether installers should include one.
4. **Signing/notarization.** Steps exist but need certificates and Apple credentials.
5. **Multi-arch container.** The Dockerfile is CLI-only and single-arch; a Chromium/Node image is still pending.

## Local Windows build evidence (2026-11-19)

`npm run build` with fetched sidecars produced:

| Artifact | Size |
| --- | --- |
| `Coqui_0.1.0_x64_en-US.msi` | 71.67 MiB |
| `Coqui_0.1.0_x64-setup.exe` (NSIS) | 40.95 MiB |

Bundled sidecars included `sidecars/node.exe` (~79.5 MB uncompressed) and the Lighthouse CLI tree; both are below the 350 MB installer budget after compression. Artifacts were unsigned.
