# Project status — 2026-11-20

Snapshot of the Coqui build for the milestone review.

## Repository

| Metric | Value |
| --- | --- |
| Open issues | 75 |
| Closed issues | 11 (including A-02, P-05, U-04, C-08) |
| Commits since 2026-11-18 | 525 |
| Main CI | green (PR #326 run) |

## Shipped

- **Crawl:** URL/sitemap/list seeds, regex scope with testers, robots.txt, redirects, auth headers, **cookies**, **custom User-Agent/Accept-Language**, **HTTP/SOCKS proxy**, checkpoints, profiles, completion webhooks, per-host delay and max-RPS, 429 retries.
- **Render:** Chromium/CDP, screenshots, console, axe, Web Vitals, custom JS, resource blocking, selector-based wait strategy.
- **Exports/reports:** 14 CSV families (incl. anchors), XLSX, sitemaps (split/gzip/image), HTML and PDF reports (summary, issues, status, performance, redirects, broken-links, overview, hreflang, hreflang-return, canonical-chains, canonicals, pagination, orphans, segments, SERP) with print styles and batch generation, filename tokens.
- **Audit:** 48 static rules (incl. DOM size, render-blocking, payload and images) plus enumerated `axe_*` rules per violation; suppression (rule + per-URL, reason required, optional expiry, actor); severity overrides; audit log; settings export/import/diff.
- **Results UI:** filters, tabs, saved views, history, overview cards (status, severity, response stats, throughput, indexability, depth, content, segments, duplicates), issue drill-down, inspector (links/images/axe/headers/screenshot/**raw HTML source viewer**).
- **Automation:** CLI (`crawl`, `resume`, `report`, `export`, `issue-settings`, `sitemap`, `schedule`, `serve`, `overview`, `prune`, `doctor`, `mcp`), MCP server, scheduler, HTTP API (bearer auth, health states, exports/reports, start crawl), Dockerfile, GitHub/GitLab CI recipes, **JSON/NDJSON crawl output with crawlId correlation**, `--fail-on-issues` exit code, comparison CSVs with per-URL issue deltas, PDF and batch reports.
- **Packaging:** Windows MSI 71.67 MiB / NSIS 40.95 MiB locally with Node + Lighthouse sidecars; macOS/Linux release jobs; **gated signing steps** (Windows signtool, macOS codesign/notarization) that activate when secrets exist.

## Decisions settled

1. Scheduler: **interval + external trigger** (no cron parser); documented CI/cron usage.
2. Raw HTML: **opt-in** `store_html` (schema v31) with an inspector source viewer.
3. Signing: **CI placeholders wired**; certificates to be supplied later.

## Next candidates

- U-03: full source virtualisation and rendered-DOM diff polish.
- X-04: asset discovery (scripts/styles/fonts, media metadata, file sizes).
- X-03/X-02: link-graph queries/anchors; near-duplicate and readability analysis.
- T-04: multi-arch image with Chromium/Node and structured stderr logs.
- G-04: storage settings and active-crawl blocking.
- B-01: full reference report catalogue.
## Recent verification (2026-11-21)

- `cargo test --workspace` green; Next.js static export compiles.
- `coqui doctor`: database, node, lighthouse and browser all ok; schema user_version 33.
- Old-schema migration test (`upgrades_old_schema_on_open`) passes.
- Portable DB copy opened, overview/report/export all worked.
- Real-site rendered crawl (adamgreenwell.com, 20 URLs): 20/20 vitals stored, 104 issues, 40.5 s; report in `docs/validation/2026-11-21-rendered-vitals.md`.
