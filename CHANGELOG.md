# Changelog

All notable changes to Coqui are documented here. The format follows Keep a Changelog and Semantic Versioning (see `docs/versioning.md`).

## [Unreleased]

### Added
- Screaming Frog `.dbseospider` import (`coqui import sf`, `--pages-only`) and link export through `unique_urls` ids.
- `coqui browser install/path` with checksum verification, permission/symlink-preserving extraction and cache discovery.
- `coqui update` and `coqui update --install` (checksum-verified self-update).
- `coqui bench` storage benchmark with query p95, budget checks and nightly regression gating.
- OS/arch-qualified release binaries with per-platform SHA256SUMS; reproducible build environment (SOURCE_DATE_EPOCH, path remapping).
- Golden-fixture parity harness with CI gate, tolerance rules and per-release reports.
- Social meta extraction (Open Graph/Twitter), image title extraction, image `title_known` migration.
- Tab-page p95 benchmark, batch URL insertion API.

### Changed
- Missing title/meta/H1 rules now cover error pages; redirect records are excluded.
- `?replytocom=` links are skipped, matching the reference crawler.
- Content types are normalised to lowercase.

### Fixed
- Importer link resolution, external anchors, rollback and temp-file handling.
- Benchmark baseline advancement only after successful runs.

## [0.1.0]

Initial public cut: crawl, audit (50 static rules + axe), reports, exports, desktop shell, API server and scheduler.
