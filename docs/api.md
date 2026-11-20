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
| GET | `/crawls/{id}/urls` | Paged URLs; query params `page`, `limit` (1–500), `query` |

Example:

```bash
coqui serve --out research/validation-122 --addr 127.0.0.1:4321
curl http://127.0.0.1:4321/health
curl http://127.0.0.1:4321/crawls
curl "http://127.0.0.1:4321/crawls/<id>/urls?limit=2"
```

Verified locally: `/health` returned `ok`, `/crawls` returned the validation crawl (`totalUrls=40`), `/urls?limit=2` returned 2 rows and `/issues` returned 15 rule rows.

## Notes

- Writes (starting crawls, exports) are not exposed yet.
- The server uses the same SQLite database as the CLI/app; running it while a crawl writes is safe for reads.
