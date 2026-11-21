# Changelog

All notable changes to Coqui are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org); the policy is in [docs/versioning.md](docs/versioning.md).

## [Unreleased]

### Added

- **Crawl engine:** URL, sitemap and list seeds (sitemap indexes, gzip), regex include/exclude scope with live testers, robots.txt (allow/disallow, wildcards, `$`), manual redirect chains, basic auth, custom headers, cookies, custom User-Agent and headers (including Accept-Language), HTTP/SOCKS proxy, bounded concurrency, per-host crawl delay and max-RPS caps, 429 retry with Retry-After, pause/resume with checkpoints, crawl profiles, completion webhooks (CLI, app, schedules).
- **Rendering:** headless Chromium over CDP, JPEG screenshots, console/exception/network capture, axe-core accessibility, Web Vitals (LCP, CLS, TBT, TTFB, FCP), custom JavaScript evaluation, asset blocking, early-load and selector-based wait strategies, authenticated rendering (cookies, headers, basic auth, user agent).
- **Audit:** 50 static rules plus one enumerated `axe_*` rule per accessibility violation; PSI-derived checks (DOM size, render-blocking requests, transfer payload, image bytes, viewport); rule and per-URL suppression with required reasons, optional expiry and actor attribution; severity overrides; audit log.
- **Results workspace (desktop):** crawl form with scope/auth/proxy/rendering settings and a searchable settings reference, result grid with status/content/depth/indexability filters, sortable columns and page size, saved views, crawl history, upload-free profiles, scheduled crawls, crawl comparison, overview cards (statuses, severity totals, average/p95 response, throughput timeline, indexability, depth, content types, top segments, duplicates), issue drill-down, URL inspector (fields, axe, headers, extracted/custom data, screenshot, in/out links, images, raw and rendered source viewer), copy/deep-link and per-view exports.
- **Exports and reports:** 15 export families (`urls`, `issues`, `links`, `headers`, `images`, `a11y`, `structured`, `vitals`, `console`, `text`, `markdown`, `extractions`, `custom-js`, `anchors`, `social`, plus `all`) and a 16-entry export catalogue for the app; HTML and PDF for 15 report kinds (summary, issues, status, performance, redirects, broken links, overview, hreflang, hreflang-return, canonical chains, canonicals, pagination, orphans, segments, SERP) plus batch generation and comparison CSVs; filename tokens and skip-empty in the app.
- **Sitemaps:** single XML, split gzip bundles, image entries, `lastmod`.
- **Automation:** CLI (`crawl`, `resume`, `report`, `reports`, `export`, `extractions`, `sitemap`, `schedule`, `serve`, `overview`, `prune`, `doctor`, `bench`, `browser`, `update`, `import sf`, `issue-settings`, `mcp`), stdio MCP server with tools, resources and prompts, interval scheduler with run-due, issue-settings export/import/diff, JSON/NDJSON output with `crawlId` correlation, `--fail-on-issues` exit code 3, prune/delete guards during running crawls.
- **HTTP API:** read-only crawl, issues, filtered/paged URLs, link edges, CSV export and HTML report endpoints; optional bearer token (`--token` / `COQUI_API_TOKEN`); stateful `/health`; `POST /crawls` to start a crawl with automatic audit; Dockerfile and Docker Compose recipes.
- **Parity and benchmarks:** Screaming Frog `.dbseospider` import (`coqui import sf`, `--pages-only`) with `unique_urls` link resolution; golden-fixture parity harness with tolerance rules, CI gate and per-release report; `coqui bench` storage benchmark with page-p95, budget checks, nightly regression gating and trend artifacts.
- **Packaging and releases:** OS/arch-qualified CLI binaries with per-platform SHA-256 manifests; Windows NSIS/MSI, macOS DMG and Linux AppImage/DEB jobs; bundled Node + Lighthouse sidecars; checksummed Chrome for Testing installer (`coqui browser install`); `coqui update` and checksum-verified `coqui update --install`; reproducible-build environment (`SOURCE_DATE_EPOCH`, path remapping); signing steps wired behind secrets.

### Changed

- Content types are normalised to lowercase in storage and comparison.
- `response_time_ms` excludes Chromium render time (HTTP only), so `slow_response`, average and p95 are accurate on rendered crawls.
- Word counts skip `<script>`, `<style>` and `<noscript>` text and count visible words only; the aggregate now matches the reference crawl within 0.4% (1.000 on the exact-scope crawl).
- Missing title/meta/H1 rules now cover error pages too; redirect records are excluded.
- `?replytocom=` comment-reply links are skipped, matching the reference crawler.
- Issue CSV/XLSX exports honour severity overrides and suppressed rules/URLs, matching reports and the UI.
- Crawl database schema is **v35** with additive migrations (saved-view sort/page size, suppressions with reasons/expiry/per-URL scope, overrides, audit log with actor, stored URL segments, raw HTML storage, image titles, social metadata).
- The web UI is React 19 + Next.js 16 static export with hand-written CSS; the earlier TanStack/Radix/Tailwind/Monaco/d3 plan was not adopted.

### Fixed

- Missing v22–v26 migrations that left existing databases without new tables/columns; migration ordering when altering tables created by earlier migrations; migrations are covered by a regression test.
- Screaming Frog import: link joins through `unique_urls`, external anchors, rollback and temporary-file cleanup on failure.
- Benchmark baselines only advance after a successful run.

### Known gaps

Not implemented yet: third-party integrations (Search Console, GA4, PageSpeed Insights, Ahrefs, Majestic, Moz), AI providers and embeddings, the reference product's 34-tab workspace and 97-report catalogue, locales, Postgres/server mode, projects/workspace lifecycle, SERP and scripted-login modes, bundled Chromium, and a signed release. See [docs/status.md](docs/status.md) for the current snapshot.

## [0.1.0] — unreleased

Initial cut: desktop shell, Rust crawl engine, 50 audit rules, results grid, CLI, reports, exports, HTTP API, scheduler and MCP server. No tag or published release yet.
