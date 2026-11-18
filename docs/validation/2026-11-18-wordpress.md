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
| Runs | three 3,000-URL validation runs |
| Status (clean run) | 2,999 × 200, 1 × 404, 0 blocked |
| Issues (clean run) | 2,472 titles > 60 chars, 2 missing meta, 2 multiple H1 |
| Throughput | ~8–19 URLs/s at concurrency 20–40 |

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


