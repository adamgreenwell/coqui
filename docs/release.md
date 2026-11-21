# Releases

**No tagged release has been published yet.** This document describes the release pipeline and how to verify an artifact once one exists.

Tagging `v*` triggers [`.github/workflows/release.yml`](../.github/workflows/release.yml), which publishes:

- OS/arch-qualified CLI binaries — `coqui-linux-x86_64`, `coqui-macos-aarch64`, `coqui-windows-x86_64.exe` (native runner arch only) — each with `SHA256SUMS-<os>-<arch>.txt`;
- desktop installers for Windows (NSIS/MSI), macOS (DMG) and Linux (AppImage/DEB) with `SHA256SUMS-windows.txt`, `SHA256SUMS-macos.txt` and `SHA256SUMS-linux.txt`;
- a parity report attached when the parity workflow runs for the release.

## Versioning

The version is the workspace version in `Cargo.toml`; every crate inherits it and `coqui --version` reports it. The bump policy, schema compatibility and release checklist are in [versioning.md](versioning.md). Release notes live in `CHANGELOG.md`.

## Reproducible builds

Every build job sets:

- `SOURCE_DATE_EPOCH` to the tagged commit timestamp;
- `RUSTFLAGS=--remap-path-prefix=$GITHUB_WORKSPACE=.` so workspace paths do not leak into binaries;
- `cargo build --release --locked -p coqui-cli`, with dependencies pinned by `Cargo.lock`.

Rebuilding the same tag with the same toolchain should produce byte-identical CLI binaries. Desktop bundles embed signatures/timestamps, so treat them as reproducible in contents rather than bytes.

## Signing

- **Windows:** `signtool` signs NSIS/MSI installers when `WINDOWS_CERTIFICATE` and `WINDOWS_CERTIFICATE_PASSWORD` are present; signatures are verified with `signtool verify /pa`.
- **macOS:** Tauri signs and notarizes with `APPLE_*` secrets; the workflow verifies with `codesign --verify --deep --strict` and `xcrun stapler validate`.
- **Linux:** packages are unsigned; publish your own detached signature or verify the checksum file.

The steps no-op when secrets are absent, so tag builds keep working unsigned. See [signing.md](signing.md).

## Verifying a download

```bash
curl -LO https://github.com/adamgreenwell/coqui/releases/latest/download/coqui-linux-x86_64
curl -LO https://github.com/adamgreenwell/coqui/releases/latest/download/SHA256SUMS-linux-x86_64.txt
sha256sum -c SHA256SUMS-linux-x86_64.txt
```

## After the first release

1. Confirm every expected asset and checksum file is attached.
2. Run `coqui update` and `coqui update --install` against the published release on each OS.
3. Record installer sizes against the 350 MB budget.
4. If certificates are configured, verify signatures rather than only checksums.
