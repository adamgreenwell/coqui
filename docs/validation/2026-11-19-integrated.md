# Integrated validation — adamgreenwell.com (2026-11-19)

Exercises rendering, custom extraction, custom JavaScript, page-text storage, accessibility, Web Vitals, exports and reports in one crawl.

## Command

```bash
coqui crawl url https://adamgreenwell.com/ \
  --max-urls 40 --render --a11y --vitals --store-text \
  --extract "h1text=h1" --js "document.title" \
  --out research/validation-79
```

Then:

```bash
coqui export --out research/validation-79
coqui extractions --out research/validation-79
coqui report summary --out research/validation-79
```

## Results

| Metric | Value |
| --- | --- |
| Crawled | 40 |
| Discovered | 127 |
| Issues | 87 |
| Wall time | 68.8 s (rendered, a11y, vitals, text) |
| Status codes | 39 × 200, 1 × 301 |
| Custom JS rows | 39 (all rendered HTML pages) |
| Custom extraction rows | 39 |
| Page text rows | 39 |
| Web Vitals rows | 40 |
| axe rows | 40 |
| Export | `urls.csv` 17.6 KB, streaming export 40 rows |
| Extractions CSV | 39 rows |
| Summary report | `report-summary.html` (40 URLs) |

Sample custom-JS values: `"Adam Greenwell - Obsessive WordPress Developer"`, `"Viewport Details - Adam Greenwell"`.

## Issue distribution

| Rule | Count |
| --- | --- |
| accessibility_violations | 39 |
| slow_ttfb | 16 |
| title_too_long | 14 |
| meta_description_too_long | 6 |
| poor_lcp | 3 |
| duplicate_meta_description | 3 |

## Observations

- Every rendered HTML page produced custom-JS, extraction and text rows; the one 301 redirect correctly produced none.
- Web Vitals and axe were captured for all 40 URLs, including the redirect.
- Rendering dominated runtime (~1.7 URLs/s), consistent with the benchmark suite.
- `coqui export` currently writes the streaming URLs CSV only; richer families (a11y, vitals, custom JS, images, headers) are produced from the desktop app's export view. CLI wrappers for those families remain a follow-up.

## Follow-ups

- Add CLI subcommands (or `--family`) for the remaining CSV export families.
- Re-run without `--a11y`/`--vitals` to compare rendered throughput.
