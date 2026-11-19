# Rule evidence — adamgreenwell.com (2026-11-19)

Re-run of the integrated validation after adding status, orphan, no-follow, mixed-content, duplicate-content, duplicate-H1 and misspelling rules, with page text stored.

## Command

```bash
coqui crawl url https://adamgreenwell.com/ \
  --max-urls 40 --render --a11y --vitals --store-text \
  --extract "h1text=h1" --js "document.title" \
  --out research/validation-122
```

## Results

| Metric | Value |
| --- | --- |
| Crawled / discovered | 40 / 127 |
| Issues | 132 |
| Wall time | 65.1 s |
| Statuses | 39 × 200, 1 × 301 |

### Issues by rule

| Rule | Count |
| --- | --- |
| accessibility_violations | 39 |
| slow_ttfb | 20 |
| slow_response | 17 |
| title_too_long | 14 |
| nofollow_internal_link | 11 |
| meta_description_too_short | 9 |
| meta_description_too_long | 6 |
| poor_lcp | 3 |
| duplicate_meta_description | 3 |
| duplicate_h1 | 3 |
| title_too_short | 2 |
| pagination_broken | 2 |
| redirected_url | 1 |
| missing_meta_description | 1 |
| layout_shift | 1 |

No duplicate-content, misspelling, mixed-content, orphan, canonical-target or 4xx/5xx issues were found on this site (all pages are unique, HTTPS, have inbound links and resolve 2xx).

## Bug found: render time counted as response time

The first run reported `slow_response` for **39/39** pages and 152 issues. The crawl timer wrapped the whole `fetch` call, so rendered crawls included Chromium render time in `response_time_ms`. The timer is now captured after the HTTP body is read and before rendering, so `response_time_ms` (and the `slow_response`/p95/avg stats) reflect HTTP only.

After the fix: `slow_response` dropped to **17**, total issues to **132**, and `slow_ttfb` remained 20 — consistent with a genuinely slow server on some pages.
