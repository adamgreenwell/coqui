# Releases

Tagging `v*` triggers `.github/workflows/release.yml`, which publishes:

- CLI binaries for Windows, macOS and Linux (`coqui` / `coqui.exe`) with `SHA256SUMS.txt`;
- desktop installers for Windows (NSIS/MSI), macOS (DMG) and Linux (AppImage/DEB) with per-platform `SHA256SUMS-windows.txt`, `SHA256SUMS-macos.txt` and `SHA256SUMS-linux.txt`.

## Reproducible builds

Every build job sets:

- `SOURCE_DATE_EPOCH` to the tagged commit timestamp;
- `RUSTFLAGS=--remap-path-prefix=$GITHUB_WORKSPACE=.` so workspace paths do not leak into binaries;
- `cargo build --locked` for the CLI, with dependencies pinned by `Cargo.lock`.

Rebuilding the same tag with the same toolchain should produce byte-identical CLI binaries. Desktop bundles also embed signatures/timestamps, so treat them as reproducible in contents rather than bytes.

## Signing

- **Windows**: `signtool` signs NSIS/MSI installers when `WINDOWS_CERTIFICATE` and `WINDOWS_CERTIFICATE_PASSWORD` secrets are present; signatures are verified with `signtool verify /pa`.
- **macOS**: Tauri signs and notarizes with `APPLE_*` secrets; the workflow verifies with `codesign --verify --deep --strict` and `xcrun stapler validate`.
- **Linux**: packages are currently unsigned; publish your own detached signature or verify `SHA256SUMS.txt`.

## Verifying a download

```bash
curl -LO https://github.com/adamgreenwell/coqui/releases/latest/download/coqui
curl -LO https://github.com/adamgreenwell/coqui/releases/latest/download/SHA256SUMS.txt
sha256sum -c SHA256SUMS.txt
```

