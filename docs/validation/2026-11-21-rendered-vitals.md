# 2026-11-21 — rendered crawl with vitals and axe on adamgreenwell.com

End-to-end validation of the new renderer metrics and rules against a real site.

## Command

```bash
coqui crawl url https://adamgreenwell.com --max-urls 20 --render --vitals --a11y \
  --out research/parity/adam-rendered
```

## Result

- **20 crawled, 55 discovered, 104 issues, 40.5 s** (rendered, headless Chromium).
- **20/20 URLs stored Web Vitals** (no render failures).

## Sample vitals (homepage)

```json
{"lcpMs":752,"cls":0.029,"tbtMs":5,"ttfbMs":434,"fcpMs":660,
 "domNodes":413,"renderBlocking":10,"transferBytes":532409,
 "resourceCount":25,"imageBytes":0,"hasViewport":true}
```

## Issue counts

| Rule | URLs |
| --- | --- |
| `render_blocking_requests` | 20 |
| `axe_aria_allowed_attr` | 20 |
| `slow_ttfb` | 11 |
| `title_too_long` | 10 |
| `slow_response` | 9 |
| `axe_heading_order` | 7 |
| `nofollow_internal_link` | 6 |
| `axe_color_contrast` | 6 |
| `meta_description_too_long` | 4 |
| `meta_description_too_short` | 3 |
| `pagination_broken` | 2 |
| `axe_label` | 2 |
| `title_too_short` | 1 |
| `poor_lcp` | 1 |
| `axe_select_name` | 1 |
| `axe_button_name` | 1 |

## Findings

- **Enumerated axe rules are working**: distinct `axe_*` rules appear instead of one aggregate accessibility rule.
- **Render-blocking is prevalent**: every page reports blocking scripts/stylesheets (WordPress theme/plugin assets in the head); the Resource Timing `renderBlockingStatus` path is being used (the same page reports 10).
- **TTFB** is elevated on 11 pages (Cloudflare + origin), consistent with the earlier HTTP-only crawl.
- **DOM size and viewport** are healthy (413 nodes, viewport present) so those rules stay quiet.
- `imageBytes` is 0 on this sample even though transfer is ~520 KB; images on these pages are lazy/background-driven, so no `improve_image_delivery` issues — a fixture with real `<img>` payloads (see `research/svg-test`) does trigger it.

## Artifacts

- Database/CSV: `research/parity/adam-rendered/`
- Earlier HTTP-only baseline: `research/parity/adam-coqui/`
