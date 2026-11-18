# Validation — WordPress sites, 2026-11-18

Real-site validation of the crawl engine: `adamgreenwell.com` (small) and `landscapemanagement.net` (large, behind Cloudflare). Wiki summary: [Validation](https://github.com/adamgreenwell/coqui/wiki/Validation).

## Method

- Harness: `cargo run -p coqui-crawl --example audit -- <kind> <seed> <out_dir> <max_urls> <concurrency>`
- Seeds: each site's published sitemap; robots.txt respected.
- Large site: an authorised browser session (Edge), cookies and exact UA written to `.secrets/cloudflare.txt` (gitignored). The harness applies them in memory only; they are not written to the crawl DB.

## adamgreenwell.com (WordPress + Yoast)

| Metric | Value |
| --- | --- |
| Sitemap | `/sitemap.xml` (Yoast index, 3 sub-sitemaps, 120 `<loc>` entries) |
| URLs stored | 173 (120 seeds + 53 discovered) |
| Content types | 164 `text/html`, 8 `image/webp`, 1 `image/png` |
| Status | 170 × 200, 3 × 301 (stale sitemap entries) |
| Canonicals | 123 present, 0 mismatching |
| Titles / meta | 161 titles / 125 meta descriptions of 164 HTML pages |
| Issues | 36 missing meta description, 23 titles over 60 characters |
| Throughput | 173 URLs in 54 s at concurrency 4 |
| Crawl DB | 2.3 MB |

## landscapemanagement.net (WordPress on Kinsta, Cloudflare)

Cloudflare challenges plain HTTP clients, including a browser user-agent. A real Edge session passed automatically; its cookies + UA enabled an authorised crawl.

| Metric | Value |
| --- | --- |
| Sitemap | `/sitemap_index.xml`, 24 sub-sitemaps, 16,704 URLs (15,904 unique seeds) |
| Runs | three 3,000-URL runs, one 18,000-URL cookie crawl, one 28,809-URL allowlisted crawl |
| Status (clean run) | 2,999 × 200, 1 × 404, 0 blocked |
| Issues (clean run) | 2,472 titles > 60 chars, 2 missing meta, 2 multiple H1 |
| Throughput | ~8–19 URLs/s at concurrency 20–40; concurrency 60 triggered 429 rate limiting, 24 was stable |

### Storage change: string links → URL ids

| Run | Pages | Link rows | DB size |
| --- | --- | --- | --- |
| v3 string links | 3,000 | 342,269 | 253.7 MB |
| v4 URL-id links (pilot) | 500 | 57k | 13.6 MB |
| v4 URL-id links (403-truncated run) | 3,000 | 179,503 | 32.2 MB |
| v5 URL-id links (clean run) | 3,000 | 342,269 | 56.4 MB |

Against the original 253.7 MB, the clean v5 run is ~4.5× smaller while storing twice as many link rows. The link table now references `url_map` ids instead of full URL strings plus a three-column unique index. The `to_id` index is a known 11 MB optimisation target.

### Session lifetime and challenge handling

The second 3,000-URL run started near expiry: 1,574 pages returned 200, then 1,426 returned Cloudflare 403 challenge pages. Before R-08 this produced 223 false `broken_internal_link` issues.

R-08 now:

- marks 403/503 challenge responses as `blocked`, without extracting content;
- aborts a crawl once 20 challenges are seen and they exceed half of all responses (checkpoint kept for resume);
- excludes blocked targets from the audit;
- keeps Cookie/Authorization headers out of the persisted crawl config (memory only).

Validation: with a refreshed session the full 3,000-URL run completed with **0 blocked** and a clean issue set. With the stale session, the crawl aborted after 63 challenge responses instead of storing 1,426 bogus rows.

### Full crawl (18,000 URL cap)

| Metric | Value |
| --- | --- |
| URLs crawled | 18,000 (cap reached) |
| Discovered | 24,761 (cap hit before full discovery) |
| Status | 15,955 × 200, 2,000 × 403 (blocked), 18 × 301, 1 × 302, 3 × 404, 23 errors |
| Time | 27.9 min (10.7 URLs/s at concurrency 24) |
| Links | 1,798,582 |
| DB | 272.7 MB (links 99 MB + indexes 119 MB + URL map 32 MB) |
| Issues | 10,778 long titles, 4,107 long descriptions, 652 duplicate titles, 553 duplicate descriptions, 281 missing meta, 55 short titles, 46 missing canonical, 39 multiple H1, 19 missing H1, 6 broken links, 1 missing title, 1 thin page |

The session cookie survived ~16,000 URLs, then 2,000 tail requests became challenges. Because blocked responses were only 11% of the crawl, the original half-of-crawl abort rule did not fire; the threshold now also aborts at 100 blocked responses when they exceed 10% of the crawl.

### Allowlisted full crawl (no cookies)

After adding the crawler egress IP to Cloudflare's allowlist, the site no longer challenged plain HTTP clients. No cookie was supplied.

| Metric | Value |
| --- | --- |
| URLs crawled | 28,809 (frontier exhausted) |
| Discovered | 29,002 |
| Status | 25,767 × 200, 1,901 × 301, 17 × 302, 1,114 × 404, 7 × 410, 3 errors, 0 blocked |
| Time | 31.9 min (15.1 URLs/s) |
| Links | 2,600,990 |
| URL map | 101,993 URLs |
| DB | 383.2 MB (links 143 MB, indexes 172 MB, URL map 37 MB, URLs 13 MB) |
| Issues | 11,919 long titles, 5,098 duplicate titles, 4,776 long descriptions, 3,820 missing meta, 3,169 missing canonical, 1,530 duplicate descriptions, 546 short titles, 272 broken links, 39 multiple H1, 18 missing H1, 13 canonical mismatches |

The remaining depth budget stopped 193 discovered links from being crawled. Duplicate titles, missing canonicals and missing descriptions cluster on archive/pagination templates; 301/404 rows are legacy URLs still linked from live pages. Dropping `idx_links_crawl_to` would save ~84 MB.

### Expanded audit pass

Re-analysing the 3,000-page crawl with the new rules produced:

| Rule | Count |
| --- | --- |
| title_too_long | 2,472 |
| meta_description_too_long | 714 |
| duplicate_title | 43 |
| duplicate_meta_description | 40 |
| missing_meta_description | 2 |
| multiple_h1 | 2 |
| missing_h1 / canonical_mismatch / missing_canonical / title_too_short | 0 |

The 714 over-long descriptions are a genuine WordPress excerpt problem; the 43 duplicate titles and 40 duplicate descriptions point at archive/template pages worth reviewing.

### Hreflang, pagination and index trim

A 3,000-page run with the rebuilt extractor found:

- **0 pages with hreflang** — this site is English-only, so the hreflang rules had no input.
- **1 page with `rel=next`** — `/2021-gieexpo-preview-the-show-goes-on/` links to `/2/`, which returns 404. The new `pagination_broken` rule caught it.

Dropping `idx_links_crawl_to` reduced the same crawl from 56.4 MB to **45.3 MB** (~20%). Existing databases need a `VACUUM` to reclaim the freed pages.

## Scale test (synthetic, local)

| URLs | Crawl time | Rate | DB |
| --- | --- | --- | --- |
| 3,000 | 4.7 s | 636/s | 1.8 MB |
| 10,000 | 16.8 s | 597/s | 6.3 MB |

## Actions

1. **R-08 follow-up**: OS keychain storage and cookie refresh; challenge classification, abort and UI warning are done.
2. Run the full 16,704-URL crawl with a fresh/allowlisted session (a 3,000-URL crawl fits within cookie lifetime; 16.7k likely does not).
3. Extend audit rules (canonical mismatch, missing H1, hreflang, pagination).
4. Classify assets (X-04) so images stop counting as HTML URL rows.
5. Incremental frontier for 100k-scale crawls (C-01).






