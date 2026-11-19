# Code signing runbook

Release artifacts are currently **unsigned**. This runbook documents exactly what changes once certificates exist; nothing here can be exercised without them.

## Current state

| Platform | Artifact | Signed |
| --- | --- | --- |
| Windows | NSIS `.exe` | No |
| macOS | `.dmg` | No |
| Linux | `.AppImage`, `.deb` | No |

Unsigned installers trigger SmartScreen/ Gatekeeper warnings. The release workflow builds and uploads them; it does not sign.

## Windows (Authenticode)

### Requirements

- An OV/EV code-signing certificate (`.pfx` or a cloud signing service such as Azure Trusted Signing).
- Timestamp server (e.g. `http://timestamp.digicert.com`).

### GitHub secrets

| Secret | Purpose |
| --- | --- |
| `WINDOWS_CERTIFICATE` | Base64 of the `.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | `.pfx` password |

For cloud signing, store the service credentials instead and call the provider's signing tool.

### Tauri configuration (sketch)

```json
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "<thumbprint>",
      "digestAlgorithm": "sha256",
      "timestampUrl": "http://timestamp.digicert.com"
    }
  }
}
```

Keep the thumbprint out of the repository for cloud setups; Tauri also supports a custom `signCommand`.

### Release step

```yaml
- name: Decode certificate
  if: runner.os == 'Windows'
  shell: pwsh
  run: |
    [IO.File]::WriteAllBytes("cert.pfx", [Convert]::FromBase64String("${{ secrets.WINDOWS_CERTIFICATE }}"))
- name: Sign installer
  if: runner.os == 'Windows'
  run: signtool sign /f cert.pfx /p "$env:WINDOWS_CERTIFICATE_PASSWORD" /tr http://timestamp.digicert.com /td sha256 /fd sha256 src-tauri/target/release/bundle/nsis/*.exe
```

### Verify

```powershell
Get-AuthenticodeSignature .\Coqui_*.exe | Format-List Status, SignerCertificate
```

## macOS (Developer ID + notarization)

### Requirements

- Apple Developer Program membership.
- **Developer ID Application** certificate installed in the runner keychain (`.p12`).
- App-specific password or App Store Connect API key for notarization.

### GitHub secrets

| Secret | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64 `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID` | Apple ID for notarytool |
| `APPLE_PASSWORD` | App-specific password |
| `APPLE_TEAM_ID` | Team identifier |

API-key alternative: `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_PATH`.

### Tauri configuration (sketch)

```json
{
  "bundle": {
    "macOS": {
      "signingIdentity": "Developer ID Application: Name (TEAMID)",
      "entitlements": "entitlements.plist"
    }
  }
}
```

Notarization can run via Tauri's built-in `notarize` support or a `xcrun notarytool submit --wait` step on the `.dmg`, followed by `xcrun stapler staple`.

### Verify

```bash
codesign --verify --deep --strict --verbose=2 "Coqui.app"
spctl --assess --type execute --verbose=4 "Coqui.app"
xcrun stapler validate Coqui_*.dmg
```

## Linux (optional)

Linux does not require signing to install. Optional hardening:

- GPG-sign the `.AppImage` (`gpg --detach-sign`) and publish the `.asc`.
- Sign `.deb` repositories with `reprepro`/`aptly` if serving an apt repo.

## CI wiring checklist

1. Add the secrets above to the repository (or environment).
2. Extend the `desktop` (Windows) job to decode and sign before the size check.
3. Extend `desktop-macos` to install the certificate, sign the `.app`/`.dmg`, notarize and staple.
4. Verify signatures in CI as a gate (`signtool verify`, `codesign --verify`, `spctl`).
5. Keep unsigned development builds separate from tag builds.

## Validation status

Nothing in this runbook is exercised by CI yet. Until certificates are provided, release notes must state that installers are unsigned.
