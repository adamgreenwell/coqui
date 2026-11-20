# HTTP API

Read-only JSON API over a Coqui crawl database, for dashboards, CI checks and integrations.

```bash
coqui serve --out DIR [--addr 127.0.0.1:8787]
```

It binds loopback by default and has **no authentication**; do not expose it publicly without a proxy.

## Endpoints

| Method | Path | Returns |
| --- | --- | --- |
| GET | `/health` | `ok` |
| GET | `/crawls` | Crawl summaries (latest 100) |
| GET | `/crawls/{id}` | One crawl summary |
| GET | `/crawls/{id}/issues` | Issue counts by rule/severity (suppression-aware) |
| GET | `/crawls/{id}/issues/{rule}` | URLs and details for one rule |
| GET | `/crawls/{id}/urls` | Paged URLs; query params `page`, `limit` (1–500), `query`, `status`, `statusClass`, `contentClass`, `maxDepth`, `indexability`, `sort`, `descending` |
| GET | `/crawls/{id}/export/{family}` | CSV download for a family (urls, issues, links, headers, images, a11y, structured, vitals, console, text, markdown, extractions, custom-js) |
| GET | `/crawls/{id}/report/{kind}` | HTML report (summary, issues, status, performance, redirects, broken-links, overview) |

Example:

```bash
coqui serve --out research/validation-122 --addr 127.0.0.1:4321
curl http://127.0.0.1:4321/health
curl http://127.0.0.1:4321/crawls
curl "http://127.0.0.1:4321/crawls/<id>/urls?limit=2"
```

Verified locally: `/health` returned `ok`, `/crawls` returned the validation crawl (`totalUrls=40`), `/urls?limit=2` returned 2 rows, and `?status=200&contentClass=html&sort=url&descending=true` applied the same filters as the grid (39 status-200 rows, 40 HTML) and `/issues` returned 15 rule rows; `/issues/title_too_long` returned 14 URLs with details. `/export/vitals` returned 200 with 3,124 bytes of `text/csv`; `/report/overview` returned 200 with 1,211 bytes of `text/html` and an unknown kind returned 400.

## Notes

- Writes (starting crawls, exports) are not exposed yet.
- The server uses the same SQLite database as the CLI/app; running it while a crawl writes is safe for reads.




