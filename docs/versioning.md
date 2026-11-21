# Versioning and compatibility

Coqui follows [Semantic Versioning](https://semver.org) (`MAJOR.MINOR.PATCH`). The current version is **0.1.0** and no tagged release has been published yet.

## What changes a version

| Change | Bump |
| --- | --- |
| Incompatible CLI flags, a store schema change that requires manual migration, removed report kinds or export families | **MAJOR** |
| New commands, audit rules, report kinds, export families, additive schema migrations | **MINOR** |
| Crawl, audit, report, performance, documentation and packaging fixes | **PATCH** |

Pre-releases use a `-rc.N` suffix (`v0.2.0-rc.1`). `coqui update` uses GitHub's `releases/latest`, which excludes pre-releases, so release candidates never become the update target.

## Release checklist

1. Update `CHANGELOG.md` (move `Unreleased` entries under the new version).
2. Bump the workspace version in `Cargo.toml`. Every crate inherits `version.workspace = true`; `coqui --version` reads it from the binary.
3. Run `cargo fmt --all --check`, `cargo test --workspace`, `npm run build:web` and `node tools/docs/check.mjs`.
4. Tag `vX.Y.Z` and push the tag. `.github/workflows/release.yml` builds and publishes the CLI binaries and desktop installers with checksums.
5. Verify the downloaded assets against `SHA256SUMS-<os>-<arch>.txt` before announcing.

There are no tags or published releases yet, so step 3 is currently the whole release gate.

## Database compatibility

- One SQLite file is one crawl; `PRAGMA user_version` records the schema version. The current version is **35** (`SCHEMA_VERSION` in `coqui-store`).
- Migrations are additive and run automatically when a database is opened. Existing crawl data is preserved across upgrades; there is no export/import step.
- **Policy:** a database written by a newer schema must be refused rather than opened. That refusal is not implemented yet; it is tracked in the backlog. Until it lands, running an older binary against a newer crawl file is unsupported and may lose data.
- Portable crawl files and the `.dbseospider` import are additive; an imported crawl is a normal Coqui crawl afterwards.

## CLI, API and file formats

- CLI flags are additive within a major version. Removing or renaming one is a major change.
- HTTP API JSON fields are additive; clients should ignore unknown fields. The API has no version prefix yet, so treat it as preview.
- Report kinds and export family ids are stable within a major version. New ones may be added in a minor release.
- CSV columns may gain new columns in a minor release; existing columns keep their meaning and order.
- The SQLite schema is an implementation detail: no external tool should write to it, and the version number may change in a minor release.

## Update channels

- **Stable:** published GitHub releases. `coqui update` compares the running version with the latest release; `coqui update --install` downloads the matching OS/arch binary, verifies its SHA-256 against `SHA256SUMS-<os>-<arch>.txt` and replaces the executable.
- **CI builds:** workflow artifacts (benchmarks, parity reports) are not releases and are never offered as updates.
