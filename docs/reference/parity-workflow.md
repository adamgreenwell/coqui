# Reference crawl parity workflow

1. Run the reference crawl (Screaming Frog 25.1) on a site and export CSVs to a folder, e.g. `research/reference/crawl-adam-25.1`.
2. Run Coqui against the same URL with equivalent settings (same mode, same depth, rendering off for an HTTP-only baseline):

   ```bash
   coqui crawl url https://adamgreenwell.com --max-urls 100 --out research/parity/adam-coqui
   ```

3. Compare exports:

   ```bash
   node tools/parity/compare-exports.mjs \
     --coqui research/parity/adam-coqui \
     --reference research/reference/crawl-adam-25.1 \
     --out research/parity/report.md
   ```

The report lists each reference CSV, its URL column, row counts, URL overlap with Coqui, and which headers Coqui's `urls.csv` shares (with a synonym map: `Address`→`url`, `Status Code`→`status`, `Page Title`→`title`, `Meta Description`→`meta_description`, `H1-1`→`h1`, `Word Count`→`words`, `Response Time`→`response_ms`).

## Interpreting gaps

- **Missing URLs** — scope/normalisation differences (trailing slash, redirects, canonical handling, robots, pagination).
- **Reference-only headers** — extraction fields Coqui does not yet export (e.g. target size, mobile viewport, PSI metrics). Map them to planned rules/tabs.
- **Issue names** — use `docs/reference/sf-25.1-issues.csv` (136 reference issue types) to align Coqui rule ids and severities; `axe` types are enumerated individually in the reference while Coqui currently aggregates accessibility.
- **Reports** — `docs/reference/sf-25.1-reports.csv` lists the 35 reference report menu entries.

## Golden fixtures

Copy a small reference export (e.g. 20–50 URLs) into `research/reference/` and keep it out of the installed app's licence state. These become read-only fixtures for future comparisons; the site must be publicly reachable to regenerate.
## Comparing link sets

Both the imported reference crawl and Coqui crawls use the same store schema, so links can be compared directly:

```bash
node tools/parity/compare-links.mjs \
  --left research/import-test2/coqui.db \
  --right research/parity/adam-coqui-v2/coqui.db \
  --out research/parity/links-report.md
```

The report counts internal HTML-to-HTML links in both, links only on each side, and samples. Restrict both sides to a common crawl scope for meaningful numbers (the reference crawl is larger than a 100-URL sample).
## One-command CI gate

The comparison runner can fail a build when overlap drops or shared-column values change:

```bash
node tools/parity/compare-exports.mjs \
  --reference tools/parity/fixtures/reference \
  --coqui tools/parity/fixtures/coqui \
  --fail-under-ratio 1 \
  --max-mismatches 0
```

- `--fail-under-ratio N` exits 3 when fewer than N of reference URLs overlap Coqui.
- `--max-mismatches N` exits 3 when more than N shared-column values differ.
- `--tolerance FILE` applies JSON rules per column: `{"type":"ignore"}` or `{"type":"numeric","absolute":N}`.
- `--compare-values-for a.csv,b.csv` limits value comparison to page-level exports (default `urls.csv`), avoiding collisions such as the image `title` attribute.
- `tools/parity/fixtures/` holds committed reference/Coqui CSV pairs (urls, links, images, social, headers, structured, console) and tolerance rules; CI runs them on every push and PR (Parity fixtures gate).


