# Handover and next steps

Snapshot of the Coqui build, the parity workflow and the work that follows it.

## What exists

- **Rust workspace:** `coqui-core` (domain, URL identity, config, issue taxonomy), `coqui-store` (SQLite schema **v35**, migrations, paged queries), `coqui-crawl` (frontier, HTTP, robots, seeds, extraction), `coqui-audit` (**50 static rules** + enumerated `axe_*`), `coqui-render` (Chromium/CDP, screenshots, axe, vitals), `coqui-lighthouse`, `coqui-report` (15 report kinds), `coqui-api` (axum), `coqui-cli`.
- **Desktop:** Tauri 2 shell with the Next.js static-export workspace (overview, results grid, inspector, settings, profiles, history, comparison, schedules).
- **Automation:** CLI, HTTP API, stdio MCP server and interval scheduler over the same SQLite crawl file.
- **Packaging:** Windows NSIS/MSI, macOS DMG and Linux AppImage/DEB release jobs, Node + Lighthouse sidecars, checksummed Chrome for Testing installer, self-update, reproducible-build environment.
- **Parity:** Screaming Frog `.dbseospider` import, golden fixtures and CI gate, benchmark budgets and nightly regression.

## Merge policy

Every PR needs:

1. Green CI on the head commit.
2. A completed Codex review on that exact head with no findings. After fixes, comment `@codex review` and wait for the review state to reference the new SHA.
3. Documentation updated in the same PR when behaviour, counts, commands or formats change. `node tools/docs/check.mjs` is the guard; it is part of CI.

## Verification (2026-11-21)

- `cargo test --workspace`: 112 pass, 5 ignored (browser-dependent render tests).
- `cargo fmt --all --check`: clean.
- CLI smoke: `coqui crawl url http://127.0.0.1:4173 --max-urls 15` against the local fixture produced 5 URLs and 21 issues; `overview`, `report summary` and `export --family all` were exercised on the same database.
- `coqui doctor`: database, Node and system Chrome detected on this machine; Lighthouse reports missing until `npm run sidecars` runs.
- Real-site validation: [regression crawl](validation/2026-11-20-regression.md), [rendered vitals](validation/2026-11-21-rendered-vitals.md), [first parity diff](validation/2026-11-21-first-parity-diff.md).

## Reference parity assets

These live under `research/` and are **not committed** (see `.gitignore`), so a fresh clone cannot rerun the comparison until the reference crawl is copied in.

- `research/reference/crawl-adam-25.1/` — Screaming Frog 25.1 export: `.dbseospider`, extracted `sf-crawl.db`, `reference-urls.csv`.
- `research/parity/adam-coqui/` — HTTP-only Coqui baseline for adamgreenwell.com (100 URLs, 248 issues, 14 CSVs).
- `research/parity/adam-rendered/` — rendered baseline (20 URLs, vitals + axe).
- Committed fixtures under `tools/parity/fixtures/` are the CI gate and do not need `research/`.

Method and evidence: [reverse-engineering.md](reverse-engineering.md), [screaming-frog-25.1.md](reference/screaming-frog-25.1.md), [parity-workflow.md](reference/parity-workflow.md), [sf-25.1-issues.csv](reference/sf-25.1-issues.csv).

### Rerunning the comparison

```bash
node tools/parity/sf-crawl-db.mjs \
  research/reference/crawl-adam-25.1/sf-crawl.db \
  research/reference/crawl-adam-25.1/reference-urls.csv
node tools/parity/compare-exports.mjs \
  --coqui research/parity/adam-coqui \
  --reference research/reference/crawl-adam-25.1 \
  --out research/parity/report.md
```

The committed fixture gate is:

```bash
node tools/parity/compare-exports.mjs \
  --reference tools/parity/fixtures/reference \
  --coqui tools/parity/fixtures/coqui \
  --tolerance tools/parity/fixtures/tolerance.json \
  --require-counterparts --max-mismatches 0
```

## Recommended next slices

1. **Newer-schema refusal** — finish the guard and its test (PR #365 is red because only the error type and test landed), then tag the first release.
2. **Reference diff follow-through** — resolve remaining scope/normalisation differences and the residual per-page word-count and link-classification gaps.
3. **Mobile family** — tap targets, illegible font size, mobile alternate link, unsupported plugins, content size (viewport is done).
4. **PSI remainder** — font display, LCP discovery, legacy JS, cache lifetimes, duplicated JS, network dependency tree; most need CSS/JS/resource analysis.
5. **Virtualised source and results** — finish U-03 for very large documents and the 1M-row grid.
6. **Asset discovery** — scripts/styles/fonts, media metadata, video/iframe inventory.
7. **Scripted login/CSRF and list/CSV import** — C-06 and P-04.
8. **Multi-arch container** — Chromium/Node in the image and structured stderr logs.

## Open decisions

- **Licence:** MIT or Apache-2.0, before the first release. No `LICENSE` file exists yet.
- **Chromium:** bundle it in installers (size budget 200–350 MB) or keep system discovery plus the checksummed installer.
- **Server mode:** whether Postgres and multi-user operation stay planned or are cut from 1.0.
- **UI breadth:** whether to build the full 34-tab model or continue with a smaller workspace that covers the same analyses.
