# Audit rules

Coqui evaluates every crawled URL and stores issues as `(rule, url, severity, detail)`. Rules are registered statically; adding one needs no migration. Suppression (rule or per-URL, reason required, optional expiry) and severity overrides apply at read/report time and are audited.

## Static rules (50)

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
| `image_missing_title` | Image missing title | Opportunity |
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

## Thresholds and triggers

Rules run after the crawl, over HTML pages unless noted. A page is “OK HTML” when it is `text/html` (or XHTML), not blocked, and returns 2xx. Exact severities are in the table above and in `RULES` (`crates/coqui-audit/src/lib.rs`).

| Rule | Trigger |
| --- | --- |
| `missing_title` | OK HTML with an empty title; error pages with no title are also flagged |
| `title_too_long` | Title length > 60 characters |
| `title_too_short` | Title length < 30 characters |
| `missing_meta_description` | Empty meta description; error pages too |
| `meta_description_too_long` | Meta description > 155 characters |
| `meta_description_too_short` | Meta description < 70 characters |
| `missing_h1` | `h1_count == 0` (error pages too) |
| `multiple_h1` | More than one `<h1>` |
| `duplicate_h1` | Two or more pages share the same trimmed H1 (case-insensitive) |
| `duplicate_title` | Two or more OK HTML pages share the same title (case-insensitive) |
| `duplicate_meta_description` | Two or more OK HTML pages share the same description |
| `duplicate_content` | Two or more pages have identical normalised stored text of at least 200 characters; requires `--store-text` |
| `misspelling` | Stored page text contains one of 25 common misspellings; requires `--store-text` |
| `thin_content` | Word count < 200 |
| `url_too_long` | URL length > 115 characters |
| `non_https` | URL starts with `http://` |
| `noindex_page` | Meta robots contains `noindex` |
| `canonical_mismatch` | Normalised canonical differs from the page URL |
| `missing_canonical` | No non-empty canonical link |
| `canonical_to_non_200` | Canonical target is not 2xx, or was not crawled |
| `redirected_url` | 3xx response |
| `long_redirect_chain` | More than 2 hops in the redirect chain |
| `client_error` | 4xx response |
| `server_error` | 5xx response |
| `broken_internal_link` | An internal link target returns ≥400 or failed; blocked, 403 and 429 targets are exempt |
| `nofollow_internal_link` | A page has internal links whose `rel` contains `nofollow` |
| `orphan_url` | 2xx page at depth > 0 with zero inbound links |
| `blocked_by_robots` | The crawler recorded the URL as blocked |
| `mixed_content` | HTTPS page references an `http://` image |
| `image_missing_alt` | At least one image has no alt text |
| `image_missing_dimensions` | At least one image has no width or height |
| `image_missing_title` | At least one image has no title |
| `jsonld_malformed` | Structured-data extraction reported an `invalid…` error |
| `jsonld_missing_required` | Structured-data extraction reported a `missing …` field |
| `jsonld_missing` | No JSON-LD types and no errors |
| `hreflang_invalid_target` | Hreflang target is missing, blocked or non-2xx |
| `hreflang_missing_return` | Target is 2xx but does not link back |
| `pagination_broken` | `rel=next`/`rel=prev` target is missing, blocked or non-2xx |
| `slow_response` | HTTP response time > 1,000 ms (render time excluded) |
| `large_page` | Response body > 1,048,576 bytes |
| `poor_lcp` | LCP > 2,500 ms |
| `layout_shift` | CLS > 0.1 |
| `slow_ttfb` | TTFB > 800 ms |
| `long_blocking_time` | TBT > 200 ms |
| `dom_size_large` | DOM nodes > 1,500 |
| `render_blocking_requests` | More than 2 blocking scripts/stylesheets |
| `avoid_enormous_network_payload` | Transfer size > 2,000,000 bytes |
| `improve_image_delivery` | Image bytes > 1,000,000 |
| `viewport_not_set` | No viewport meta tag |
| `axe_*` | One issue per axe violation; severity from impact (above) |

Suppression and severity overrides are applied at read/report time, so the table describes what the crawler records, not necessarily what the UI shows.

