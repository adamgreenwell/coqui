# Validation — WordPress sites, 2026-11-18

First real-site validation of the M3 crawl engine: `adamgreenwell.com` (small) and `landscapemanagement.net` (large, behind Cloudflare).

## Method

- Harness: `cargo run -p coqui-crawl --example audit -- <kind> <seed> <out_dir> <max_urls> <concurrency>`
- Seeds: each site's published sitemap, robots.txt respected.
- Large site: an authorised browser session (Edge), from which the site's cookies and exact user-agent were copied to `.secrets/cloudflare.txt` (gitignored). The harness applies them in memory only; they are not written to the crawl database.
- Audit: the six M3 rules; CSV inspected directly.

## adamgreenwell.com (WordPress + Yoast)

| Metric | Value |
| --- | --- |
| Sitemap | `/sitemap.xml` (Yoast index, 3 sub-sitemaps, 120 `<loc>` entries) |
| URLs stored | 173 (120 sitemap seeds + 53 discovered links) |
| Content types | 164 `text/html`, 8 `image/webp`, 1 `image/png` |
| Status | 170 × 200, 3 × 301 (stale sitemap entries) |
| Canonicals | 123 present, 0 mismatching |
| Titles | 161 of 164 HTML pages (3 missing are 301 rows) |
| Meta descriptions | 125 of 164 HTML pages |
| Issues | 36 missing meta description, 23 titles over 60 characters |
| Throughput | 173 URLs in 54 s at concurrency 4 |
| Crawl DB | 2.3 MB |

## landscapemanagement.net (WordPress on Kinsta, Cloudflare)

The site returns a Cloudflare managed challenge to HTTP clients, including the browser user-agent. A real Edge session passed the challenge automatically. Cookies + user-agent from that session were used for an authorised crawl.

| Metric | Value |
| --- | --- |
| Sitemap | `/sitemap_index.xml`, 24 sub-sitemaps, 16,704 `<loc>` entries (15,904 unique seeds after normalisation) |
| Crawl scope | capped at 3,000 URLs for this validation |
| Status | 2,999 × 200, 1 × 404 |
| Content | 3,000 HTML pages; canonical on 2,999; meta description on 2,997 |
| Issues | 2,472 titles over 60 characters, 3 missing meta description, 2 multiple H1 |
| Links | 342,269 rows (114 per page, 54,996 external) |
| Throughput | ~9 URLs/s resumed at concurrency 40 (site latency dominates) |
| Crawl DB | 253.7 MB |

Observations:

- Titles average 79 characters, largely because of the `| Landscape Management` suffix. This is a genuine finding for the site, not a crawler artefact.
- The database is dominated by the link graph: 342k link rows with full source/target URL strings and a three-column unique index account for ~245 MB of the 254 MB. Before full 16k-URL crawls, links must store URL ids, not strings.
- Sitemap URLs are image sitemaps too; nested `<image:loc>` entries were initially mistaken for page seeds. Fixed: only the page `<loc>` inside `<url>` is seed material, with a regression test.
- Cloudflare cookies are valid for a limited window; a full crawl needs either a longer-lived authorised policy or cookie refresh. The crawler does not persist them.

## Scale test (synthetic, local)

Generated sitemap + pages (`tools/fixture/bulk.mjs`).

| URLs | Crawl time | Rate | DB size |
| --- | --- | --- | --- |
| 3,000 | 4.7 s | 636/s | 1.8 MB |
| 10,000 | 16.8 s | 597/s | 6.3 MB |

The synthetic fixture has almost no links per page, so its DB size is not comparable to a real WordPress site.

## Actions

1. **Link storage by URL id** (P-01/X-03): the dominant storage and insert cost on real sites. This is the blocker for comfortable 16k+ crawls.
2. **Incremental frontier** (C-01): the checkpoint still serialises seen URLs as JSON; acceptable at 16k, not at 100k.
3. **R-08**: cookie-based authorised sessions now work; still needs challenge classification, secret storage and UI support.
4. **A-01**: add canonical mismatch, missing H1, hreflang and pagination rules; the current six found real issues but miss most WordPress SEO surface.
5. **X-04**: classify images/assets so they stop appearing as HTML URL rows.
6. Run the full 16,704-URL crawl once link storage is fixed.
