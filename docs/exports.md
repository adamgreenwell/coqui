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

Exports currently load the crawl rows into memory before writing. Streaming for very large crawls (1M rows) is still pending.
