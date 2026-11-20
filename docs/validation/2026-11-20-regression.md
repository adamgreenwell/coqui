# Regression crawl — adamgreenwell.com (2026-11-20)

Post-change regression after the response-time fix, schema v29, per-host throttle, segment breakdown and issue-settings work.

## Command

```bash
coqui crawl url https://adamgreenwell.com/ \
  --max-urls 20 --render --a11y --vitals --store-text \
  --delay-ms 100 --out research/regression-158
```

## Results

| Metric | Value |
| --- | --- |
| Crawled / discovered | 20 / 55 |
| Issues | 65 |
| Wall time | 39.6 s (37.8 s crawl) |
| Statuses | 20 × 200 |

### Issues by rule

| Rule | Severity | Count |
| --- | --- | --- |
| accessibility_violations | warning | 20 |
| slow_ttfb | opportunity | 10 |
| title_too_long | warning | 10 |
| slow_response | warning | 8 |
| nofollow_internal_link | warning | 6 |
| meta_description_too_long | warning | 4 |
| meta_description_too_short | opportunity | 3 |
| pagination_broken | error | 2 |
| poor_lcp | warning | 1 |
| title_too_short | opportunity | 1 |

### Segments

plugins 7, tools 5, (root) 1, about 1, then individual post slugs.

## Notes

- No duplicate-content, mixed-content, canonical-target, 4xx/5xx or orphan issues on this sample.
- `slow_response` stays low relative to `slow_ttfb`, consistent with the timer fix that excludes render time.
- Per-host 100 ms delay applied; ~0.53 rendered URLs/s with Chromium, axe and vitals enabled.
- Report generation (`coqui report summary`) produced HTML for the 20 URLs.
