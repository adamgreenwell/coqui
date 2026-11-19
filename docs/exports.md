# Exports

Coqui exports the current results grid and the crawl's bulk data.

## Views

| View | Formats |
| --- | --- |
| URLs (active issue filter + visible columns) | CSV, XLSX |
| Issues | CSV, XLSX |
| Links | CSV, XLSX |
| Response headers | CSV |
| Images | CSV |
| Accessibility violations | CSV |
| Structured data | CSV |
| Web Vitals | CSV |
| Console errors | CSV |
| Screenshots | folder bundle (JPEGs + `screenshots.csv`) |
| Sitemap | XML (indexable URLs + image entries) |
| Sitemap bundle | folder of split gzipped sitemaps + sitemap-index.xml |
| HTML report | self-contained report; types: summary, issues, status, performance |

## Catalogue

`coqui exports` lists the named export catalogue (`id`, `format`, `label`, description). The same catalogue is exposed to the app as the `export_catalog` Tauri command, so menus can be generated instead of hard-coded.

IDs currently include: `urls-csv`, `urls-xlsx`, `issues-csv`, `issues-xlsx`, `links-csv`, `links-xlsx`, `headers-csv`, `images-csv`, `accessibility-csv`, `structured-csv`, `vitals-csv`, `console-csv`, `screenshots`.

## Filename patterns

The pattern field accepts:

| Token | Value |
| --- | --- |
| `{date}` | `YYYY-MM-DD` (UTC) |
| `{datetime}` | `YYYYMMDD-HHMMSS` (UTC) |
| `{domain}` | Seed host |
| `{crawl}` | Crawl id |
| `{view}` / `{report}` | Export view name |

Default pattern: `{crawl}-{view}-{datetime}`.

Values are sanitised (path separators and unsafe characters become `-`), and the format extension is appended when missing. Example:

```text
{domain}-{view}-{date}  ->  example.com-urls-2026-11-19.csv
```

## Skip empty

With **Skip empty** enabled, an export with zero rows is discarded and no path is returned. Useful for scheduled/bulk runs where empty reports should not create files.

## Limits

Sitemaps include `<lastmod>` parsed from each page's `Last-Modified` header when present. Bundles split at 50,000 URLs, write `sitemap-index.xml` pointing at `sitemap-N.xml.gz`, and gzip each part. Single sitemap generation includes only 200 responses without `noindex`, caps at 50,000 URLs, and emits `<image:image>` entries from the image inventory.

URL CSV exports stream row-by-row from SQLite, so they do not hold the crawl in memory. XLSX exports and the data exports (headers, images, a11y, structured, vitals, console) still materialise their rows.







