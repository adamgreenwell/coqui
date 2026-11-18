# Validation — WordPress sites, 2026-11-18

First real-site validation of the M3 crawl engine. Two sites were requested: `adamgreenwell.com` (small) and `landscapemanagement.net` (large). The small site crawled end to end; the large site is behind a Cloudflare managed challenge and could not be fetched with an HTTP client.

## Method

- Harness: `cargo run -p coqui-crawl --example audit -- <kind> <seed> <out_dir> <max_urls> <concurrency>`
- Seeds: the site's own published sitemap (`sitemap` kind), robots.txt respected, default Coqui user agent.
- Post-crawl audit: the six M3 rules. CSV export inspected directly.

## adamgreenwell.com (WordPress + Yoast)

| Metric | Value |
| --- | --- |
| Sitemap | `/sitemap.xml` (Yoast index, 3 sub-sitemaps, 120 `<loc>` entries) |
| Crawl seed handling | Sitemap index returned a 301; now followed (fixed during validation) |
| URLs stored | 173 (120 sitemap seeds + 53 discovered links) |
| Content types | 164 `text/html`, 8 `image/webp`, 1 `image/png` |
| Status codes | 170 × 200, 3 × 301 (stale sitemap entries) |
| Canonicals | 123 present, 0 mismatching the crawled URL |
| Titles | 161 of 164 HTML pages (the 3 without are 301 rows) |
| Meta descriptions | 125 of 164 HTML pages |
| Issues | 36 missing meta description, 23 titles over 60 characters |
| Throughput | 173 URLs in 54.1 s at concurrency 4 (3.2 URLs/s) |
| Crawl DB | 2.3 MB |

Observations:

- The 301s are sitemap entries that redirect; Coqui records them as redirect rows and discovers the target, matching expected crawler behaviour.
- Missing meta descriptions are real: tags, author archive, contact and two documentation pages. This is exactly the sort of finding the audit should surface.
- The 9 image URLs were reached through in-content links. They are stored as URL rows but excluded from content rules by content-type. They belong in the Images tab (X-04), not the HTML issue set.
- Canonicals were consistently self-referential, so no canonical checks fire. The current rule set does not yet check canonical mismatch because it is not implemented.

## Scale test (synthetic, local)

A generated sitemap + pages fixture (`tools/fixture/bulk.mjs`) was used to test throughput and storage without external load.

| URLs | Crawl time | Rate | DB size | Issues |
| --- | --- | --- | --- | --- |
| 3,000 | 4.7 s | 636/s | 1.8 MB | 0 |
| 10,000 | 16.8 s | 597/s | 6.3 MB | 0 |

At 10k URLs the current per-batch checkpoint (full frontier + seen JSON) is still acceptable locally, but it scales O(n) per batch and should be replaced with incremental tables before 100k-URL crawls or slow networks.

## landscapemanagement.net (WordPress on Kinsta, behind Cloudflare)

| Request | Result |
| --- | --- |
| `/robots.txt` | 200 |
| `/` (apex and www) | 403, Cloudflare challenge (~1.4 KB page) |
| `/sitemap_index.xml` | 403 |
| Browser user agent + browser headers | 403 (same challenge) |
| Coqui user agent | 403 |

The site is protected by a managed challenge that requires JavaScript execution and/or an allowlisted client. This is not an HTTP-client problem to work around silently: the correct behaviours are to detect the challenge, report it clearly, and support an authorised session.

## Actions

1. **R-08** (new): challenge/access-denied handling — detect WAF challenge responses, classify them separately from content, and allow an authorised session (cookies/headers) for staging or allowlisted crawls.
2. **X-04**: classify non-HTML assets and keep them out of the HTML URL/issue counts; add image-specific columns.
3. **A-01**: extend the rule set with canonical mismatch, missing H1, hreflang and pagination checks — the current six rules found real issues but miss most WordPress SEO surface.
4. **C-07**: sitemap redirect following now works; add a regression test with a 301 sitemap.
5. Ask for a Cloudflare allowlist for `Coqui/0.1` (or a staging URL) to run the large-site crawl.
