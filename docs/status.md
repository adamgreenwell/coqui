# Project status — 2026-11-20

Snapshot of the Coqui build for the milestone review.

## Repository

| Metric | Value |
| --- | --- |
| Open issues | 76 |
| Closed issues | 10 (including A-02 overrides/suppression, P-05 comparison, U-04 overview) |
| Commits since 2026-11-18 | 370 |
| Main CI | green (PR #246 run) |

## Shipped

- **Crawl:** URL/sitemap/list seeds, regex scope, robots.txt, redirects, auth headers, checkpoints, profiles, completion webhooks, per-host delay and max-RPS, 429 retries.
- **Render:** Chromium/CDP, screenshots, console, axe, Web Vitals, custom JS, resource blocking.
- **Audit:** 44 rules; suppression (rule + per-URL, reason required, optional expiry, actor); severity overrides; audit log; settings export/import/diff.
- **Results UI:** filters, tabs, saved views, history, overview cards (status, severity, response stats, throughput, indexability, depth, content, segments), issue drill-down, inspector (links/images/axe/headers/screenshot).
- **Exports/reports:** 13 CSV families, XLSX, sitemaps (split/gzip), HTML/PDF reports (summary, issues, status, performance, redirects, broken links, overview, comparison), filename tokens.
- **Automation:** CLI (`crawl`, `resume`, `report`, `export`, `issue-settings`, `sitemap`, `schedule`, `serve`, `doctor`, `mcp`), MCP server, scheduler, HTTP API (bearer auth, exports/reports, start crawl), Dockerfile, CI recipes.
- **Packaging:** Windows MSI 71.67 MiB / NSIS 40.95 MiB locally with Node + Lighthouse sidecars; macOS/Linux release jobs; signing runbook.

## Decisions pending

1. Scheduler: cron syntax vs interval + external trigger.
2. Code signing/notarization: certificates.
3. Raw HTML storage for the source viewer (opt-in flag).

## Next candidates

- U-03 duplicates pane; source viewer once decision 3 is made.
- P-05 URL-level diff export + 10k benchmark.
- C-08 auto-throttle/circuit breaker and crawl timeline.
- X-04 srcset/natural-size/assets.

