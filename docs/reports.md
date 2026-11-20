# Reports

HTML reports render from the latest crawl in a database; PDF is produced through headless Chrome/Edge.

```bash
coqui report <kind> --out DIR [--top N] [--status CODE] [--pdf]
coqui report all --out DIR        # every report, failures isolated
coqui report compare --with CRAWL --out DIR
```

Outputs land as `DIR/report-<kind>.html` (plus `.pdf` with `--pdf`).

## Catalogue

| Kind | Title | Contents |
| --- | --- | --- |
| `summary` | Crawl summary | Statuses, issues, slowest URLs, throughput |
| `issues` | Issue breakdown | Issue counts across the crawl |
| `status` | Status codes | Response status distribution |
| `performance` | Performance | Slowest URLs, throughput and per-second timeline |
| `redirects` | Redirects | 3xx responses with targets |
| `broken-links` | Broken links | 4xx/5xx and failed responses |
| `overview` | Crawl overview | Status, indexability, content, depth and issue totals |\n| `hreflang` | Hreflang | Hreflang link pairs per page (language and target) |\n| `canonical-chains` | Canonical chains | Canonical targets followed across the crawl, with loops |\n| `pagination` | Pagination | rel=next/prev targets, with both statuses |\n| `orphans` | Orphan pages | Indexable 200 pages with no internal inlinks (search URL excluded) |\n| `segments` | Segments overview | URL counts and share per configured segment |\n| `serp` | SERP summary | Title/meta/H1/word count with missing, long, duplicate and thin flags |\n| `canonicals` | Canonicals | Self/non-self/missing declarations with target status |\n| `hreflang-return` | Hreflang return links | Target status plus missing return-link flags |

## Options

- `--top N` limits list rows (issues/performance).
- `--status CODE` filters rows by HTTP status.
- `--pdf` also writes a PDF via headless Chrome/Edge (`COQUI_BROWSER_PATH` override; absolute paths used).
- `report all` catches per-report failures, continues, and prints written/failed counts.

## Comparison

`coqui report compare --with <crawl id>` writes:

- `report-compare.html` — side-by-side rule counts.
- `compare.csv` — `rule,left,right,delta`.
- `compare-urls.csv` — added/removed/changed URLs with before/after status, title, meta, H1 and issue counts.

## Print styles

Reports include `@page A4` with 12 mm margins and page-break avoidance for headings and table rows, so PDFs are print-friendly.








