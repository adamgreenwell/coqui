# HTTP API

Read-mostly JSON API over a Coqui crawl database, for dashboards, CI checks and integrations.

```bash
coqui serve --out DIR [--addr 127.0.0.1:8787] [--token TOKEN]
```

It binds loopback by default. Set `--token` or `COQUI_API_TOKEN` to require `Authorization: Bearer <token>` on every route except `/health`; without a token the API is unauthenticated, so do not expose it beyond loopback without a proxy.

## Endpoints

| Method | Path | Returns |
| --- | --- | --- |
| GET | `/health` | `{"status":"ok"\|"starting"\|"crawling","crawls":N}` |
| GET | `/crawls` | Crawl summaries (latest 100) |
| POST | `/crawls` | Start a crawl; 202 with `{"id": ...}` |
| GET | `/crawls/{id}` | One crawl summary |
| GET | `/crawls/{id}/issues` | Issue counts by rule/severity (suppression-aware) |
| GET | `/crawls/{id}/issues/{rule}` | URLs and details for one rule |
| GET | `/crawls/{id}/links?url=` | Inbound/outbound link edges with anchor, rel and internal flag |
| GET | `/crawls/{id}/urls` | Paged URLs with filters |
| GET | `/crawls/{id}/export/{family}` | CSV download for an export family |
| GET | `/crawls/{id}/report/{kind}` | HTML report |

`/health` is the only unauthenticated route. `/health` reports `crawling` while any crawl runs and `starting` while a submitted crawl is still pending.

## Starting a crawl

```bash
curl -X POST http://127.0.0.1:8787/crawls \
  -H 'content-type: application/json' \
  -d '{"seed":"https://example.com","seedKind":"url","maxUrls":50}'
```

`seed` is required. `seedKind` is `url` (default), `sitemap` or `list`; `maxUrls` overrides the default. The crawl runs in-process with the default `CrawlConfig`, is audited automatically on completion, and the response is HTTP 202 with `{"id":"api-<timestamp>"}`.

## URLs

`GET /crawls/{id}/urls` accepts:

| Parameter | Meaning |
| --- | --- |
| `page` | 1-based page (default 1) |
| `limit` | Page size, 1–500 |
| `query` | Substring match on URL or title |
| `status` | Exact HTTP status |
| `statusClass` | `2`, `3`, `4` or `5` (2xx…5xx) |
| `contentClass` | `html`, `image`, `xml`, … |
| `maxDepth` | Maximum depth |
| `indexability` | Indexability filter |
| `sort` | Sort column (`url`, `status`, `title`, `depth`, …) |
| `descending` | `true`/`false` |

Response: `{"rows":[…],"total":N,"page":N,"pageSize":N}` (field names camelCase).

## Exports and reports

- `GET /crawls/{id}/export/{family}` streams 14 CSV families: `urls`, `issues`, `links`, `headers`, `images`, `a11y`, `structured`, `vitals`, `console`, `text`, `markdown`, `extractions`, `custom-js`, `anchors`. The API does not expose `social` or `all`; use the CLI for those.
- `GET /crawls/{id}/report/{kind}` renders the same 15 HTML report kinds as `coqui report`; an unknown kind returns 400.

## Filters and auth example

```bash
coqui serve --out research/validation-122 --addr 127.0.0.1:4321 --token secret123
curl http://127.0.0.1:4321/health
curl -H 'Authorization: Bearer secret123' \
  'http://127.0.0.1:4321/crawls/<id>/urls?limit=2&statusClass=2&contentClass=html&sort=url&descending=true'
curl -H 'Authorization: Bearer secret123' \
  'http://127.0.0.1:4321/crawls/<id>/issues/title_too_long'
```

With token auth enabled, `/health` remains open; `/crawls` returns 401 without the header and 200 with it.

## Container

```bash
docker build -t coqui-api .
docker run --rm -p 127.0.0.1:8787:8787 -v "$PWD/data:/data" coqui-api
```

The image is CLI-only: rendering, axe and Lighthouse need a browser and Node inside the image or mounted through `COQUI_BROWSER_PATH` / sidecars. `docker compose up --build` uses the same image and binds the port to loopback.

## Notes and stability

- Starting crawls is the only write endpoint; exports and reports are read-only.
- The server shares the SQLite file with the CLI/app. Reads are safe while a crawl writes.
- The API has no `/v1` prefix yet. Treat it as preview: JSON fields are additive, and clients should ignore unknown fields. See [versioning.md](versioning.md).
- `POST /crawls` accepts only `CrawlConfig` defaults plus `maxUrls`; other crawl settings are CLI/app-only for now.
