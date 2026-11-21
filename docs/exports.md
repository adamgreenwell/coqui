# Exports

Coqui exports the current results view and the crawl's bulk data. The CLI and API cover the CSV families; the desktop app adds XLSX, a named catalogue and export preferences.

## CLI and API families

`coqui export --out DIR [--family FAMILY]` (or `GET /crawls/{id}/export/{family}`):

| Family | Contents | Notes |
| --- | --- | --- |
| `urls` | The current crawl's URL rows | Default family |
| `issues` | One row per issue (url, rule, severity, detail) | Suppression/override aware |
| `links` | Link graph (from, to, anchor, rel, internal) | |
| `headers` | Final response headers | |
| `images` | Image references with alt and dimensions | |
| `a11y` | One row per axe violation | Requires `--a11y` |
| `structured` | JSON-LD types and validation errors | |
| `vitals` | LCP, CLS, TBT, TTFB, FCP | Requires `--vitals` |
| `console` | Console and resource errors | Requires rendering |
| `text` | Extracted body text | Requires `--store-text` |
| `markdown` | Generated Markdown | Requires `--store-text` |
| `extractions` | Custom extraction values | Requires `--extract` |
| `custom-js` | Custom JavaScript results | Requires `--js` with `--render` |
| `anchors` | Anchor texts and targets | |
| `social` | Open Graph and Twitter tags | |

`--family all` writes the 15 families in one pass. Files are named `<family>-<milliseconds>.csv`; the CLI does not accept a filename pattern.

The HTTP API exposes 14 of these families (it omits `social` and `all`).

## App catalogue

The desktop app exposes a 16-entry catalogue (`export_catalog` in `coqui-core`, the `coqui exports` list, and the in-app export menu):

| ID | Format | Contents |
| --- | --- | --- |
| `urls-csv` / `urls-xlsx` | CSV / XLSX | URLs with the selected columns and active issue filter |
| `issues-csv` / `issues-xlsx` | CSV / XLSX | Issues |
| `links-csv` / `links-xlsx` | CSV / XLSX | Link graph |
| `headers-csv` | CSV | Response headers |
| `images-csv` | CSV | Images with alt and dimensions |
| `text-csv` | CSV | Page text (needs store text) |
| `markdown-csv` | CSV | Page Markdown (needs store text) |
| `extractions-csv` | CSV | Custom extraction values |
| `accessibility-csv` | CSV | Axe violations |
| `structured-csv` | CSV | Structured data |
| `vitals-csv` | CSV | Web Vitals |
| `console-csv` | CSV | Console errors |
| `screenshots` | Folder | JPEGs plus `screenshots.csv` |

`anchors`, `social` and `custom-js` are CLI/API families and do not appear in the app catalogue.

## Filename tokens (app only)

The app's export preferences accept a pattern with these tokens:

| Token | Value |
| --- | --- |
| `{date}` | `YYYY-MM-DD` (UTC) |
| `{datetime}` | `YYYYMMDD-HHMMSS` (UTC) |
| `{domain}` | Seed host |
| `{crawl}` | Crawl id |
| `{view}` / `{report}` | Export view or report name |

Default: `{crawl}-{view}-{datetime}`. Unsafe characters are replaced with `-`, the format extension is appended when missing, and an empty result falls back to `<crawl>-<view>.<ext>`.

The app also has **skip empty**: when enabled, a zero-row export writes no file and the UI reports it as skipped.

## Sitemaps

- `coqui sitemap --out DIR` writes `sitemap.xml` with 2xx, indexable URLs and `lastmod` where a `Last-Modified` header was stored.
- `--split` writes a 50,000-URL bundle under `DIR/sitemaps` with a `sitemap-index.xml`.
- `--gzip` compresses the split parts.
- `--images` includes image entries (`<image:image>`).

## Validation

- Streaming CSV was exercised to 1M rows within the 500 ms page budget for the grid; export itself is row-by-row and bounded.
- `--family all` produced 15 files on the local fixture and printed `export all: 15 families`.
- Screenshot bundles were verified on a rendered crawl; the folder contains one JPEG per URL plus `screenshots.csv`.
