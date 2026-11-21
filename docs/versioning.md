# Versioning policy

Coqui follows [Semantic Versioning](https://semver.org): `MAJOR.MINOR.PATCH`.

- **MAJOR** — incompatible CLI flags, store schema breaks that require manual migration, or removed report formats.
- **MINOR** — new commands, rules, report kinds, export families, or additive schema migrations.
- **PATCH** — fixes to crawling, auditing, reports, performance, docs, and packaging.

## Releases

1. Update `CHANGELOG.md`.
2. Bump the workspace version in `Cargo.toml` (single source: `crates/*` inherit it; the CLI reports it via `coqui --version`).
3. Tag `vX.Y.Z` and push the tag; `.github/workflows/release.yml` builds and publishes artifacts.
4. The release workflow attaches per-platform checksums; `coqui update` compares the running version with the latest release.

Pre-releases use `-rc.N` suffixes on the tag; `coqui update` ignores them for the "latest" check because GitHub's `releases/latest` excludes pre-releases.

## Compatibility

- **Database**: migrations are additive and run automatically on open (`PRAGMA user_version`). A newer database opened by an older binary is refused rather than corrupted.
- **CLI**: flags are additive within a major version; removed/renamed flags are a major-version change.
- **API**: JSON fields are additive within a major version; clients should ignore unknown fields.
- **Reports and exports**: existing columns/families stay stable within a major version.

## Update channels

- Stable: published GitHub releases (default for `coqui update`).
- Nightly/CI builds: not published as releases; benchmark and parity workflows publish artifacts only.
