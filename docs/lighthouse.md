# Lighthouse (system npx)

On-demand Lighthouse runs for a single crawled URL, using **system Node.js + npx** (option B). The app does not bundle Node or Lighthouse yet.

## Requirements

- Node.js 20+ and `npx` on `PATH`.
- A Chrome/Edge binary Lighthouse can launch. Set `CHROME_PATH` if auto-detection fails.
- Internet access the first time (`npx` downloads `lighthouse@12.8.2`).

## Behaviour

- Row action **LH** calls `run_lighthouse(crawl_id, url_id)`.
- Tauri spawns `npx --yes lighthouse@12.8.2 <url>` with a 240-second timeout, JSON output to a temp file.
- The JSON is reduced to category scores plus up to 12 failing audits and stored in `lighthouse_reports`.
- A modal shows Performance / Accessibility / Best practices / SEO scores and the failing audits.
- Missing Node surfaces as “could not run npx … Install Node.js or add it to PATH”; a Lighthouse failure returns its stderr snippet.

## Storage

Schema v12, `lighthouse_reports(crawl_id, url_id, summary_json, created_at)`. Full Lighthouse reports (~1.5 MB) are not stored; only the summary.

## Validation

Staging homepage, Lighthouse 12.8.2 via Edge:

| Category | Score |
| --- | --- |
| Performance | 72 |
| Accessibility | 95 |
| Best practices | 57 |
| SEO | 42 |

The SEO score reflects staging behaviour (no canonical/noindex), matching the crawler’s own findings. Top audits included back/forward cache, colour contrast, crawlable anchors, deprecations and forced reflow.

## Limits

- One URL at a time, manually triggered; no crawl-wide Lighthouse run.
- Lighthouse launches its own browser, so Coqui’s session cookies/headers are **not** applied — protected pages may be challenged.
- First run downloads Lighthouse (~30–60 s).

## Path to 1.0

Bundle Node + Lighthouse as a Tauri sidecar so the feature works offline and behind logins (with our session material). The command and storage contract stay the same.
