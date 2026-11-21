# Screaming Frog 25.1 reference extraction

First evidence pass against the **full licensed application** (installed 2026-11-20).

## Artifact

| Field | Value |
| --- | --- |
| Install | `C:\Program Files (x86)\Screaming Frog SEO Spider` |
| Main JAR | `ScreamingFrogSEOSpider.jar` |
| JAR SHA-256 | `3894c61083db5067dfd944d83edd70bc06f3a7a33abb7dd5a40b12cb6ffaa7ec` |
| Version | **25.1** (`project.version`) |
| Build commit | `4a68a9139d6be0ce3eee7799e71945c5cb2c28e3` |
| Runtime | JDK 25.0.5, JavaFX 25.0.6-ea |
| Main class | `uk.co.screamingfrog.seospider.SeoSpider` |

## Method

Static extraction only: the JAR was opened as a ZIP; `META-INF/MANIFEST.MF`, `project.properties` and the English translation bundles (`translations/SeoSpider_en.properties`, `Utils_en.properties`) were read. No execution or network activity.

- Translation bundle: 7,065 lines, 5,509 `key = value` pairs.
- `SeoSpider_en.properties` drives the UI; `menu.*`, `issue.*`, `tab.*` and `option.*` keys map directly to product surfaces.

## Issue taxonomy

**136 issue types** with descriptions and tips (`issue.<family>.<id>.description/.tip`).

| Family | Types | Coqui today |
| --- | --- | --- |
| `axe` | 95 | 1 aggregated `accessibility_violations` rule |
| `psi` | 13 | none (Web Vitals only) |
| `internal` | 11 | largely covered (status, redirects, robots, no-response) |
| `mobile` | 6 | none |
| `external` | 4 | partial (client/server errors only for crawled URLs) |
| `images` | 4 | alt, dimensions, mixed content |
| `links` | 2 | nofollow/uncrawlable partially |
| `directives` | 1 | none |

Extracted rows: [sf-25.1-issues.csv](sf-25.1-issues.csv).

Notable gaps to plan:

- **PSI family** — avoid enormous network payload, document latency, duplicated JavaScript, font display, forced reflow, image delivery, layout-shift culprits, LCP request discovery, legacy JS, network dependency tree, DOM size, render-blocking requests, cache lifetimes.
- **Mobile family** — content size, illegible font size, mobile alternate link, tap targets, unsupported plugins, viewport not set.
- **Axe family** — 95 enumerated WCAG rules (accesskeys, area_alt, aria_*, colour contrast, labels, landmarks, …). Coqui currently stores axe violations but reports one aggregate rule; enumerating them is a direct parity win.
- **External family** — blocked resources, no response, client/server errors for external URLs.

## Report catalogue

**35 report menu entries** extracted (`menu.reports.*`), e.g.:

Accessibility, Accessibility Violations Summary, All Redirects, Canonical Chains, Canonicals, Crawl Overview, Crawl Overview Comparison, Crawl Overview, Hreflang, Non-200 hreflang URLs, Unlinked hreflang URLs, All hreflang URLs, Missing Return Links, Noindex Return Links, Non Canonical Return Links, Insecure Content, Issues Overview, All Issues, Found in Crawl, Mobile, Target Size, Non-Indexable Canonicals, Orphan Pages, Pagination, Redirect & Canonical Chains, Redirect Chains, Redirects, Redirects to Error, Non-200 Pagination URLs, Unlinked Pagination URLs, Segments Overview, SERP Summary, Social, Non-200 OG Image URLs, Non-200 X Card Image URLs.

Extracted rows: [sf-25.1-reports.csv](sf-25.1-reports.csv).

Coqui currently ships 7 crawl reports plus comparison; the reference has 35 entries with several specialised families (hreflang, canonical chains, pagination, social, mobile, segments, SERP summary).

## Tab/filter surface

**335 `tab.*.filter.*` keys** show the reference's per-tab filter vocabulary (e.g. JavaScript tab: title only in rendered HTML, H1 updated by JS, canonical mismatch, noindex only in original HTML). These are useful for grid filter parity and saved-view presets.

## Importing the crawl file

`coqui import sf --file path/to/crawl.dbseospider --out DIR` reads the ZIP, extracts `results_*/sql`, and imports URLs and hyperlinks into a new crawl. Verified with this install: 211 internal URLs and 3,826 link rows. See `docs/cli.md`.

## Next passes

1. Run the licensed app against a local fixture and capture exported CSVs to build golden fixtures (issue columns, tab columns, report contents).
2. Enumerate axe rules as individual Coqui rules from the stored axe payload.
3. Add PSI-family rules where the data already exists (layout shift, LCP, render-blocking, DOM size) and standalone reports (hreflang, canonical chains, pagination, social, mobile, segments, SERP).
4. Replace the guessed parity inventory with reference-derived issue/report inventories.
## Link types in the crawl database

Sampling the `links` table shows what each `link_type` stores:

| Type | Meaning | Example element path |
| --- | --- | --- |
| 1 | Anchor links (`<a href>`) | `//body/.../a` |
| 2 | Script `src` resources | `//head/script` |
| 3 | Stylesheet/resource links | `//head/link` |
| 4 | Image sources | `//body/.../img` |
| 6 | Canonical link element | `//head/link[1]` |
| 13 | External anchors/resources | `//head/link[9]` |
| 15 | Analytics/beacon and scripted requests | (no element path) |
| 29/30 | Meta-driven resource references | `//head/meta` |

The importer ingests **types 1 and 13** (internal and external anchors). Resource links (2/3/4/6/29/30) stay out of the link graph, matching Coqui crawls, which only record anchors; the reference type-1 rows can still include duplicate occurrences.

