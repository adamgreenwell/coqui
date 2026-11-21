# Lighthouse

On-demand Lighthouse runs for crawled URLs, using a bundled Node + Lighthouse sidecar when present and falling back to system `npx`.

## Sidecar resolution

Coqui looks for these in order:

1. `COQUI_NODE` / `COQUI_LIGHTHOUSE_CLI` environment overrides.
2. `node` / `node.exe` and `lighthouse/lighthouse-cli/index.js` next to the Coqui executable or under `src-tauri/sidecars/`.
3. System `node` plus `npx --yes lighthouse@12.8.2`.

Release desktop builds run `npm run sidecars`, which downloads Node 22.14.0 (checksum-verified against `SHASUMS256.txt`) and installs Lighthouse 12.8.2 with production dependencies into `src-tauri/sidecars/`; the Tauri bundle includes that directory. Development builds without sidecars use `npx`.

## Requirements

- Node.js 20+ (22.14.0 when using the sidecar).
- A Chrome/Edge binary Lighthouse can launch. Set `CHROME_PATH` if discovery fails.
- Internet access the first time if `npx` has to download Lighthouse.

## Behaviour

- Row action **LH** calls `run_lighthouse(crawl_id, url_id)`.
- Tauri spawns the sidecar Node with Lighthouse 12.8.2, or falls back to `npx --yes lighthouse@12.8.2 <url>`, with a 240-second timeout and JSON output to a temp file.
- The JSON is reduced to category scores plus up to 12 failing audits, stored in `lighthouse_reports`.
- A modal shows Performance / Accessibility / Best practices / SEO scores and the failing audits.
- Missing Node surfaces as “could not run npx … Install Node.js or add it to PATH”; a Lighthouse failure returns its stderr snippet.

## Sampled audits

The Audit header button runs Lighthouse sequentially on three evenly spaced 2xx HTML URLs, stores each summary, and opens the first report with a “sampled 3 pages” label. There is no full-crawl Lighthouse run.

## Storage

`lighthouse_reports(crawl_id, url_id, summary_json, created_at)` exists in the current schema (introduced in v22). Full Lighthouse reports (~1.5 MB each) are not stored; only the summary.

## Validation

Staging homepage, Lighthouse 12.8.2 via Edge:

| Category | Score |
| --- | --- |
| Performance | 72 |
| Accessibility | 95 |
| Best practices | 57 |
| SEO | 42 |

The SEO score reflects staging behaviour (no canonical/noindex), matching the crawler's own findings. Top audits included back/forward cache, colour contrast, crawlable anchors, deprecations and forced reflow.

## Limits

- One URL at a time, or an evenly spaced sample of 3; no full-crawl run.
- Lighthouse launches its own browser, so Coqui's session cookies/headers are **not** applied — protected pages may be challenged.
- First run through `npx` downloads Lighthouse (~30–60 s); the sidecar avoids this.
- `coqui doctor` reports whether Node, the Lighthouse CLI and a browser were found.
