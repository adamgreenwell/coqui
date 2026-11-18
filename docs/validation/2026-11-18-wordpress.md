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
| Runs | two 3,000-URL validation runs |
| Status (first run) | 2,999 × 200, 1 × 404 |
| Issues (first run) | 2,472 titles > 60 chars, 3 missing meta, 2 multiple H1 |
| Throughput | ~8–19 URLs/s at concurrency 20–40 |

### Storage change: string links → URL ids

| Run | Pages | Link rows | DB size |
| --- | --- | --- | --- |
| v3 string links | 3,000 | 342,269 | 253.7 MB |
| v4 URL-id links (pilot) | 500 | 57k | 13.6 MB |
| v4 URL-id links | 3,000 | 179,503 | 32.2 MB |

That is roughly an **8× reduction** at 3,000 pages. The link table now references `url_map` ids with two indexes instead of storing full URL strings plus a three-column unique index.

### Cookie lifetime finding

The second 3,000-URL run started with a session that was already near expiry: 1,574 pages returned 200, then 1,426 returned Cloudflare 403 challenge pages. The audit initially reported 223 `broken_internal_link` issues against those challenge targets.

Two consequences:

1. The audit now ignores 403 link targets rather than calling them broken, pending R-08 classification.
2. A full 16,704-URL crawl needs a longer-lived allowlist or a cookie-refresh mechanism. The crawler does not persist cookies.

## Scale test (synthetic, local)

| URLs | Crawl time | Rate | DB |
| --- | --- | --- | --- |
| 3,000 | 4.7 s | 636/s | 1.8 MB |
| 10,000 | 16.8 s | 597/s | 6.3 MB |

## Actions

1. **R-08**: classify challenge responses, surface incomplete coverage, support secret-stored sessions and refresh.
2. Run the full 16,704-URL crawl with a fresh/allowlisted session.
3. Extend audit rules (canonical mismatch, missing H1, hreflang, pagination).
4. Classify assets (X-04) so images stop counting as HTML URL rows.
5. Incremental frontier for 100k-scale crawls (C-01).
