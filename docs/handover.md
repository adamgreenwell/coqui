# Handover and next steps

Snapshot of the Coqui build and the reference-parity workflow.

## What exists

- Rust workspace: `coqui-core`, `coqui-store` (SQLite schema v33), `coqui-crawl`, `coqui-audit`, `coqui-render`, `coqui-lighthouse`, `coqui-api`, `coqui-report`, `coqui-cli`, `src-tauri`.
- Next.js 16 static export UI (overview, results grid, inspector, settings, profiles, history).
- Desktop packaging (Windows MSI/NSIS with Node + Lighthouse sidecars; macOS/Linux release jobs; signing steps gated on secrets).
- CLI/API/MCP automation: crawl/resume/report/export/sitemap/schedule/serve/overview/prune/doctor/mcp, bearer auth, health states, link edges, JSON/NDJSON output, `--fail-on-issues`.
- Audit: **49 static rules + enumerated `axe_*`** per violation, including PSI-derived DOM size, render-blocking, transfer payload and image bytes, plus viewport detection.
- Reports: 15 kinds + comparison (summary, issues, status, performance, redirects, broken-links, overview, hreflang, hreflang-return, canonical chains, canonicals, pagination, orphans, segments, SERP).

## Merge policy

Every PR needs:

1. Green CI on the head commit.
2. A **completed Codex review on that exact head** with no findings. After fixes, comment `@codex review` and wait for the review state to reference the new SHA.

## Reference parity assets

- `docs/reference/screaming-frog-25.1.md` — licensed install (v25.1, build commit, JAR SHA-256), method, counts.
- `docs/reference/sf-25.1-issues.csv` — 136 issue types (axe 95, psi 13, internal 11, mobile 6, external 4, images 4, links 2, directives 1).
- `docs/reference/sf-25.1-reports.csv` — 35 report menu entries.
- `docs/reference/parity-workflow.md` — crawl → export → compare loop.
- `tools/parity/compare-exports.mjs` — diffs reference CSVs against Coqui `urls.csv` (URL overlap, shared/reference-only headers).
- `research/parity/adam-coqui` — HTTP-only Coqui baseline for adamgreenwell.com (100 URLs, 248 issues, 14 CSVs).
- `research/parity/adam-rendered` — rendered baseline (20 URLs, vitals + axe).

### Comparing once the reference export exists

```bash
node tools/parity/compare-exports.mjs \
  --coqui research/parity/adam-coqui \
  --reference research/reference/crawl-adam-25.1 \
  --out research/parity/report.md
```

Then map reference-only headers to planned extraction/rules and align issue ids using the reference CSV.

## Recommended next slices

1. **Run the reference diff** as soon as the Screaming Frog export lands; fix scope/normalisation and column gaps first.
2. **Mobile family** — remaining types: tap targets, illegible font size, mobile alternate link, unsupported plugins, content size (viewport done).
3. **PSI remainder** — font display, render-blocking already, LCP discovery, legacy JS, cache lifetimes, duplicated JS, network dependency tree; most need CSS/JS/resource analysis.
4. **U-03** — virtualised source rendering for very large documents (currently chunked at 200 KB with “Load more”).
5. **X-04** — asset discovery (scripts/styles/fonts), media metadata, video/iframe inventory.
6. **C-06** — scripted login/CSRF flows and session import.
7. **P-04** — local URL-list/CSV import with rejected-line reporting.
8. **T-04** — multi-arch image with Chromium/Node and structured stderr logs.

## Open decisions

- None blocking. The only external dependency is Adam's **Screaming Frog export** for the first real diff.
- Existing settled decisions: interval scheduler + external trigger; raw HTML storage opt-in; bundle Node/Lighthouse and Chromium in the installer; installer size target 200–350 MB.
