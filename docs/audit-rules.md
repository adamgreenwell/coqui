# Audit rules

Coqui evaluates every crawled URL and stores issues as `(rule, url, severity, detail)`. Rules are registered statically; adding one needs no migration. Suppression (rule or per-URL, reason required, optional expiry) and severity overrides apply at read/report time and are audited.

## Static rules (49)

| Rule | Label | Severity |
| --- | --- | --- |
| `accessibility_violations` | Accessibility violations | Warning |
| `avoid_enormous_network_payload` | Enormous network payload | Warning |
| `blocked_by_robots` | Blocked by robots.txt | Warning |
| `broken_internal_link` | Broken internal link | Error |
| `canonical_mismatch` | Canonical mismatch | Warning |
| `canonical_to_non_200` | Canonical to non-200 | Warning |
| `client_error` | 4xx client error | Error |
| `dom_size_large` | Large DOM size | Warning |
| `duplicate_content` | Duplicate content | Warning |
| `duplicate_h1` | Duplicate H1 | Warning |
| `duplicate_meta_description` | Duplicate meta description | Warning |
| `duplicate_title` | Duplicate title | Warning |
| `hreflang_invalid_target` | Hreflang target not reachable | Warning |
| `hreflang_missing_return` | Hreflang missing return link | Warning |
| `image_missing_alt` | Image missing alt text | Warning |
| `image_missing_dimensions` | Image missing dimensions | Opportunity |
| `improve_image_delivery` | Heavy image delivery | Warning |
| `jsonld_malformed` | Malformed structured data | Warning |
| `jsonld_missing` | No structured data | Opportunity |
| `jsonld_missing_required` | Structured data missing required fields | Warning |
| `large_page` | Large page | Opportunity |
| `layout_shift` | Layout shift | Warning |
| `long_blocking_time` | Long blocking time | Opportunity |
| `long_redirect_chain` | Long redirect chain | Warning |
| `meta_description_too_long` | Meta description too long | Warning |
| `meta_description_too_short` | Meta description too short | Opportunity |
| `missing_canonical` | Missing canonical | Opportunity |
| `missing_h1` | Missing H1 | Warning |
| `missing_meta_description` | Missing meta description | Warning |
| `missing_title` | Missing title | Error |
| `misspelling` | Possible misspelling | Opportunity |
| `mixed_content` | Mixed content | Warning |
| `multiple_h1` | Multiple H1 headings | Warning |
| `nofollow_internal_link` | Nofollow internal link | Warning |
| `noindex_page` | Noindex page | Warning |
| `non_https` | Non-HTTPS URL | Opportunity |
| `orphan_url` | Orphan URL | Opportunity |
| `pagination_broken` | Broken pagination link | Error |
| `poor_lcp` | Poor Largest Contentful Paint | Warning |
| `redirected_url` | Redirected URL | Opportunity |
| `render_blocking_requests` | Render-blocking requests | Opportunity |
| `server_error` | 5xx server error | Error |
| `slow_response` | Slow response | Warning |
| `slow_ttfb` | Slow server response | Opportunity |
| `thin_content` | Thin content | Opportunity |
| `title_too_long` | Title too long | Warning |
| `title_too_short` | Title too short | Opportunity |
| `url_too_long` | URL too long | Opportunity |
| `viewport_not_set` | Viewport not set | Warning |

`accessibility_violations` is retained for older databases but is no longer emitted; axe results are enumerated instead (below).

## Enumerated axe rules

When rendering with accessibility enabled, every axe-core violation becomes its own issue:

- Rule id: `axe_<violation id>` with dashes replaced by underscores (e.g. `axe_color_contrast`, `axe_image_alt`).
- Severity from axe impact: `critical`/`serious` → Error, `moderate` → Warning, `minor` → Opportunity.
- Detail: node count plus axe help text.
- UI renders these as “Axe: color contrast” style labels.

The reference product exposes 95 `axe` issue types; because Coqui enumerates axe ids dynamically, any axe rule the bundled axe-core version reports is covered without new code.

## Reference mapping

| Reference family | Types | Coqui coverage |
| --- | --- | --- |
| `internal` | 11 | Status, redirects, robots, no-response, orphan, canonical/hreflang/pagination checks |
| `images` | 4 | Alt text, dimensions, mixed content; image byte weight via `improve_image_delivery` |
| `links` | 2 | Nofollow internal links; uncrawlable links partially |
| `directives` | 1 | Not yet (directive placement beyond head) |
| `mobile` | 6 | `viewport_not_set`; tap targets/font size/alternate/plugins/content size pending |
| `psi` | 13 | `dom_size_large`, `render_blocking_requests`, `avoid_enormous_network_payload`, `improve_image_delivery`, TTFB/LCP/CLS/TBT checks |
| `external` | 4 | Client/server errors for crawled URLs only; external resource auditing pending |
| `axe` | 95 | Enumerated dynamically per violation |

## Thresholds

- LCP > 2500 ms, CLS > 0.1, TTFB > 800 ms, TBT > 200 ms.
- DOM nodes > 1,500; render-blocking resources > 2; transfer > 2 MB; image bytes > 1 MB.
- Title > 60 chars, meta description outside 70–160 chars, thin content < 200 words.

Exact severities are in the table above and in `RULES` (`crates/coqui-audit/src/lib.rs`).
