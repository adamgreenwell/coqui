# Changelog

All notable changes to Coqui are documented here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added

- **Crawl engine:** URL/sitemap/list seeds (sitemap indexes, gzip), regex include/exclude scope with live testers, robots.txt (allow/disallow, wildcards, `$`), manual redirect chains, basic auth, custom headers, cookies, custom User-Agent/Accept-Language, HTTP/SOCKS proxy, bounded concurrency, per-host crawl delay and max-RPS caps, 429 retry with Retry-After, pause/resume with checkpoints, crawl profiles, completion webhooks (CLI, app, schedules).
- **Rendering:** headless Chromium over CDP, screenshots, console capture, axe accessibility, Web Vitals, custom JavaScript evaluation, asset blocking and render wait options.
- **Audit:** 44 rules covering titles/meta, status codes (4xx/5xx/3xx), redirect chain length, canonical targets, hreflang/pagination, JSON-LD, accessibility/vitals, robots/noindex/HTTPS, duplicates (title/description/H1/content), misspellings, images (alt, dimensions, mixed content), orphan URLs and nofollow internal links. Rule and per-URL suppression with required reasons, optional expiry, and actor attribution; severity overrides; all actions audited.
- **Results UI:** search and filters (status class, content class, depth, indexability), result tabs, sortable columns, saved views (filters + sort + page size), crawl history with open/delete, crawl overview cards (status, severity totals, avg/p95 response, histograms, indexability, depth, content types, clickable top segments, duplicates), issue drill-down with example URLs and inspector links, URL inspector (fields, axe, headers, extracted/custom data, screenshot, in/out links with filter + CSV, images, copy/deep-link, raw HTML source viewer).
- **Exports/reports:** CSV/XLSX for URLs/issues/links, headers, images, a11y, structured data, vitals, console, text/Markdown, custom extraction, custom JS, anchor texts; screenshot bundles; sitemaps (single/split/gzip/image); HTML/PDF reports (summary, issues, status, performance, redirects, broken links, overview, comparison) with print styles and batch generation; filename tokens and skip-empty; streaming CSV validated to 1M rows.
- **Automation:** CLI (`crawl`, `resume`, `report`, `export --family`, `issue-settings`, `sitemap`, `schedule`, `serve`, `overview`, `prune`, `doctor`, `mcp`), stdio MCP server with tools/resources/prompts (including `export_family` and `compare_crawls`), interval scheduler with run-due, issue settings export/import/diff, two-crawl comparison CSVs (rules and URLs with issue deltas), JSON/NDJSON crawl output with crawlId correlation, `--fail-on-issues` exit code, prune/delete guards during running crawls.
- **HTTP API:** read-only crawl/issues/URLs-with-filters/link-edges/CSV-export/HTML-report endpoints, optional bearer-token auth, stateful `/health`, `POST /crawls` to start crawls with automatic audit, Dockerfile container recipe and CI operation docs (GitHub + GitLab).
- **Packaging:** Windows MSI/NSIS with bundled Node + Lighthouse sidecars (locally 71.67 MiB / 40.95 MiB), macOS/Linux release jobs, code-signing runbook and gated CI signing steps (Windows signtool, macOS codesign/notarization) that activate when secrets are configured.

### Changed

- Issue CSV/XLSX exports now honor severity overrides and exclusions from suppressed rules/URLs, matching reports and the UI.

- `response_time_ms` now excludes Chromium render time (HTTP only), making `slow_response`, average and p95 stats accurate on rendered crawls.
- Crawl database schema is v30 with additive migrations (saved-view sort/page size, suppressions with reasons/expiry/per-URL scope, overrides, audit log with actor, stored URL segments).

### Fixed

- Missing v22–v26 migrations that would have left existing databases without new tables/columns; migrations are now covered by a regression test.
- Migration ordering when altering tables created by earlier migrations.

## 0.1.0

- Initial desktop shell, crawl engine and results grid.

