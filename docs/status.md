# Project status — 2026-11-21

Snapshot of the Coqui build after the parity and release-engineering slices.

## Milestone

**M6 — parity breadth and release engineering.** The original plan defined M0 (walking skeleton) through M5 (1.0). Work has since added M6 to cover parity measurement, import compatibility, benchmarks and packaging before 1.0; no tagged release exists yet.

## Repository

| Metric | Value |
| --- | --- |
| Issues | 86 (12 epics + 74 features) |
| Open / closed | 74 / 12 |
| Commits since 2026-11-18 | 686 |
| Main CI | green (`0f13cf9`, run 44914271620) |
| Released versions | none (no tags) |
| Schema version | 35 |
| Rust tests | 112 pass, 5 browser-dependent render tests ignored |

## Shipped

- **Crawl:** URL/sitemap/list seeds, regex scope with testers, robots.txt, redirect chains, basic auth, headers, cookies, custom User-Agent and headers (including Accept-Language), HTTP/SOCKS proxy, checkpoints, profiles, completion webhooks, per-host delay and max-RPS, 429 retries.
- **Render:** Chromium/CDP, screenshots, console/network capture, axe, Web Vitals, custom JS, authenticated rendering, resource blocking and selector-based waits.
- **Audit:** 50 static rules plus one `axe_*` rule per violation; suppression (rule + per-URL, reason required, optional expiry, actor), severity overrides, audit log, settings export/import/diff.
- **Workspace:** grid filters/tabs/sort/page size, saved views, crawl history, profiles, schedules, comparison, overview cards (statuses, severity totals, average/p95, throughput, indexability, depth, content types, segments, duplicates), issue drill-down, inspector (links, images, axe, headers, screenshot, raw/rendered source), Lighthouse per URL and sampled.
- **Exports/reports:** 15 export families plus `all`, 16-entry app export catalogue, XLSX, sitemaps (single/split gzip/image), 15 HTML/PDF report kinds plus batch and comparison CSVs.
- **Automation:** CLI (`crawl`, `resume`, `report`, `reports`, `export`, `extractions`, `sitemap`, `schedule`, `serve`, `overview`, `prune`, `doctor`, `bench`, `browser`, `update`, `import sf`, `issue-settings`, `mcp`), stdio MCP server, interval scheduler, HTTP API (bearer token, health states, exports/reports, start crawl), JSON/NDJSON output, `--fail-on-issues`.
- **Parity/release:** `.dbseospider` import, golden fixtures and CI gate, benchmark budgets and nightly regression, OS/arch-qualified release assets with checksums, reproducible-build environment, Node/Lighthouse sidecars, Chrome for Testing installer, update check and self-update, Docker Compose.
- **Validation:** exact-scope parity on adamgreenwell.com (156/156 URLs; status, title, depth exact; word-count ratio 1.000), social meta 1,152/1,152 values, rendered crawl with 20/20 vitals.

## Not yet built

- Third-party integrations: Search Console, GA4, PageSpeed Insights, Ahrefs, Majestic, Moz.
- AI providers, prompts, embeddings and clustering.
- Reference breadth: 34-tab workspace, 97-report catalogue, 344 issue rules, 7 locales.
- Projects/workspace lifecycle, Postgres/server mode, multi-user operation.
- SERP mode, scripted login/CSRF flows, CSV/list import with line reporting.
- Bundled Chromium, code-signed installers (certificates pending), a tagged release.

## Known gaps and risks

1. **Newer-schema refusal is not implemented.** A database written by a newer binary can currently be opened by an older one; the draft on `feature/versioning-policy` (PR #365) is red because the guard was never committed with its test. Documented as a policy gap in [versioning.md](versioning.md).
2. **UI breadth.** The desktop workspace is one 3,878-line `page.tsx` with a single grid; it does not yet approach the reference's tab/column model, and it has no virtualisation for very large result sets.
3. **Documentation drift** is now checked by `tools/docs/check.mjs`, but generated reference data (`docs/reference/*.csv`) is a planning snapshot and its statuses are not live; the plan data lists 73 features while the board has 74 (R-08 was added later).
4. **No licence.** The repository has no `LICENSE` file; the README records the decision as pending.
5. **Local-only evidence.** `research/` artifacts referenced by validation notes and the parity workflow are gitignored, so parity reruns need the local reference crawl.

## Next candidates

From [handover.md](handover.md):

- Finish the reference diff (scope, normalisation, residual word-count/link-set gaps).
- Mobile rule family and the remaining PSI rules (font display, LCP discovery, legacy JS, cache lifetimes, duplicated JS, dependency tree).
- Virtualised source viewer and 1M-row result virtualisation.
- Asset discovery (scripts/styles/fonts, media metadata, file sizes).
- Scripted login/CSRF and list/CSV import with rejected-line reporting.
- Multi-arch container image with Chromium + Node.
- Newer-schema refusal, then the first tagged release.
