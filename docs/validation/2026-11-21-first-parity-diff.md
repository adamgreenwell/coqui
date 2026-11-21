# 2026-11-21 — first real parity diff (Screaming Frog 25.1 vs Coqui)

The Screaming Frog CLI/UI export produced a proprietary `.dbseospider`; it is a ZIP whose `results_*/sql` member is a **SQLite database**. That unlocked the first real comparison without CSV export.

## Reference data

- File: `research/reference/crawl-adam-25.1/https-adamgreenwell-com-.dbseospider`
- Extracted DB: `research/reference/crawl-adam-25.1/sf-crawl.db` (3.4 MB, 73 tables)
- `urls` table: 209 columns, 267 rows (211 internal URLs)
- Also available: `links`, `inlink_counts`, `axe_core_results`, `page_speed_api`, `social_meta_tags`, hreflang/pagination multimaps, duplicates tables, etc.

Convert it to a comparable CSV with:

```bash
node tools/parity/sf-crawl-db.mjs research/reference/crawl-adam-25.1/sf-crawl.db research/reference/crawl-adam-25.1/reference-urls.csv
node tools/parity/compare-exports.mjs --coqui research/parity/adam-coqui --reference research/reference/crawl-adam-25.1 --out research/parity/report.md
```

## Results (100 overlapping URLs)

| Field | Match | Notes |
| --- | --- | --- |
| Status | 100/100 | exact |
| Title | 100/100 | exact |
| Depth | 100/100 | exact |
| Content type | now exact | Coqui now stores lowercased content types (`charset=utf-8`) |
| Word count | 0/100 | Coqui systematically higher |

## Word count fix

Coqui counted `<script>`/`<style>`/`<noscript>` text because it used the DOM `text()` of `<body>`. Extraction now walks text nodes and skips those elements, and counts only visible words.

Homepage: **409 → 257** words (reference **222**). Across 8 pages the ratio moved from ~1.8× to **1.09–1.27×** (one code-sample page 2.6×). Residual gap is likely boilerplate/hidden text handling; the script inflation is gone.

## Link counts

Comparing Coqui's unique internal outlinks against `urls.num_unique_internal_outlinks`:

- 85 pages compared, **22 exact**, most mismatches within ±1–4 links.
- Outliers both ways (e.g. accessibility-statement +11, page/3 -4).
- The reference `links` table stores resource links (CSS/JS) with separate `link_type`/`scope` values, so the next pass must filter to hyperlink types before judging gaps.

## Follow-ups

1. Normalise `content_type` casing in comparisons (and in stored values if desired).
2. Investigate the residual 10–25% word-count gap (candidate: footer/nav or hidden elements; Screaming Frog's own definition).
3. Mine the reference DB tables directly for issue parity (`axe_core_results`, `page_speed_api`, hreflang/pagination multimaps, duplicates).
4. Re-crawl at ≥ reference scope (267 URLs) for full overlap.
## Re-run after the fixes (100 URLs)

| Field | Match |
| --- | --- |
| Status | 100/100 |
| Title | 100/100 |
| Depth | 100/100 |
| Content type (case-insensitive) | 100/100 (3 reference rows use uppercase `UTF-8`) |
| Word count (aggregate) | ratio **1.004** (mine 24,9xx vs reference 24,8xx); 3 exact per page |

Word counts now aggregate within 0.4% of the reference; per-page differences remain (rounding, boilerplate, code samples) but the script/style inflation is gone.
## Full-scope crawl (154 pages vs 156 reference HTML pages)

- **151 pages compared**: status **151/151**, title **151/151**, depth **151/151**.
- Word count aggregate ratio **1.003**; 3 exact per page.
- 3 Coqui-only URLs are `?replytocom=` comment-reply links the reference does not crawl.
- 5 reference-only URLs are depth-4 `lumenare-search/documentation/*` pages beyond our crawl depth.
- Link-set comparison still shows a gap (1,011 reference-only, 2,165 Coqui-only among 151 shared pages), consistent with different anchor de-duplication/classification in the reference’s `links` table rather than missing pages.
## Link import correction

`links.src_id`/`dst_id` reference `unique_urls.id`, not `urls.rowid`. The importer was joining the wrong table, so imported link destinations were wrong. After fixing the join and importing type 1 anchors:

- Reference import: 156 pages, **4,691 links** (3,392 internal).
- Link-set comparison vs the full Coqui crawl: **2,280 links in both** (was 415), 992 reference-only, 461 Coqui-only among 151 shared pages.
