# Reports

HTML reports render from the latest crawl in a database; PDF is produced through headless Chrome/Edge.

```bash
coqui report <kind> --out DIR [--top N] [--status CODE] [--title TEXT] [--pdf]
coqui report all --out DIR [--top N] [--status CODE]
coqui report compare --with CRAWL --out DIR [--top N] [--status CODE]
coqui reports
```

Outputs land as `DIR/report-<kind>.html` (plus `.pdf` with `--pdf`). `--pdf` is not applied by `report all`; run the kinds you need individually.

## Catalogue

| Kind | Title | Contents |
| --- | --- | --- |
| `summary` | Crawl summary | Statuses, issues and slowest URLs |
| `issues` | Issue breakdown | Issue counts across the crawl |
| `status` | Status codes | Response status distribution |
| `performance` | Performance | Slowest URLs by response time |
| `redirects` | Redirects | 3xx responses with targets |
| `broken-links` | Broken links | 4xx/5xx and failed responses |
| `overview` | Crawl overview | Status, indexability, content, depth and issue totals |
| `hreflang` | Hreflang | Hreflang link pairs per page (language and target) |
| `canonical-chains` | Canonical chains | Canonical targets followed across the crawl, with loops |
| `pagination` | Pagination | rel=next/prev targets and whether they resolve to 200 |
| `orphans` | Orphan pages | Indexable 200 pages with no internal inlinks |
| `segments` | Segments overview | URL counts per configured segment |
| `serp` | SERP summary | Title/meta/H1/word count with missing, long, duplicate and thin flags |
| `canonicals` | Canonicals | Self, non-self and missing canonical declarations |
| `hreflang-return` | Hreflang return links | Target status plus missing return-link flags |

`coqui reports` prints the same catalogue.

## Options

- `--top N` limits list rows (issues, performance).
- `--status CODE` filters rows by HTTP status.
- `--title TEXT` overrides the document title and H1.
- `--pdf` also writes a PDF via headless Chrome/Edge (`COQUI_BROWSER_PATH` overrides discovery; the output path is absolute).

## Batch generation

`coqui report all` renders every report kind, catches per-report failures, continues, and prints written/failed counts. It does not stop on the first error.

## Comparison

`coqui report compare --with <crawl id>` writes:

- `report-compare.html` — side-by-side rule counts.
- `compare.csv` — `rule,left,right,delta`.
- `compare-urls.csv` — added/removed/changed URLs with before/after status, title, meta, H1 and issue counts.

Both crawls must exist in the same database.

## Print styles

Reports include `@page A4` with 12 mm margins and page-break avoidance for headings and table rows, so PDFs are print-friendly.

## Current limits

15 kinds rather than the reference product's 97-entry report menu; there is no email delivery or scheduled report generation. The app's report view renders the same HTML.
