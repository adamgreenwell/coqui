# Project status — 2026-11-20

Snapshot of the Coqui build for the milestone review.

## Repository

| Metric | Value |
| --- | --- |
| Open issues | 76 |
| Closed issues | 10 (including A-02 overrides/suppression, P-05 comparison, U-04 overview) |
| Commits since 2026-11-18 | 414 |
| Main CI | green (PR #266 run) |

## Shipped

- **Crawl:** URL/sitemap/list seeds, regex scope with testers, robots.txt, redirects, auth headers, **cookies**, **custom User-Agent/Accept-Language**, **HTTP/SOCKS proxy**, checkpoints, profiles, completion webhooks, per-host delay and max-RPS, 429 retries.
- **Render:** Chromium/CDP, screenshots, console, axe, Web Vitals, custom JS, resource blocking.
- **Audit:** 44 rules; suppression (rule + per-URL, reason required, optional expiry, actor); severity overrides; audit log; settings export/import/diff.
- **Results UI:** filters, tabs, saved views, history, overview cards (status, severity, response stats, throughput, indexability, depth, content, segments, duplicates), issue drill-down, inspector (links/images/axe/headers/screenshot/**raw HTML source viewer**).
- **Exports/reports:** 13 CSV families, XLSX, sitemaps (split/gzip), HTML/PDF reports (summary, issues, status, performance, redirects, broken links, overview, comparison), filename tokens.
- **Automation:** CLI (`crawl`, `resume`, `report`, `export`, `issue-settings`, `sitemap`, `schedule`, `serve`, `overview`, `prune`, `doctor`, `mcp`), MCP server, scheduler, HTTP API (bearer auth, exports/reports, start crawl), Dockerfile, CI recipes, comparison CSVs with per-URL issue deltas.
- **Packaging:** Windows MSI 71.67 MiB / NSIS 40.95 MiB locally with Node + Lighthouse sidecars; macOS/Linux release jobs; **gated signing steps** (Windows signtool, macOS codesign/notarization) that activate when secrets exist.

## Decisions settled

1. Scheduler: **interval + external trigger** (no cron parser); documented CI/cron usage.
2. Raw HTML: **opt-in** `store_html` (schema v31) with an inspector source viewer.
3. Signing: **CI placeholders wired**; certificates to be supplied later.

## Next candidates

- U-03 rendered-DOM comparison; source viewer shipped.
- X-04 srcset/natural-size/asset metadata.
- C-08 auto-throttle/circuit breaker and crawl timeline.
- B-01 PDF output; P-05 and U-04 are closed.

