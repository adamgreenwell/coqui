# Coqui CLI

Headless crawler using the same engine, store and audit rules as the desktop app. First cut: `coqui crawl`.

## Usage

```bash
coqui crawl <url|sitemap|list> <seed> [--out DIR] [--max-urls N] [--depth N] [--concurrency N] [--ignore-robots] [--user-agent UA] [--header "Name: value"] [--auth user:pass] [--timeout-ms N] [--render] [--screenshots] [--a11y] [--vitals] [--render-workers N]
coqui resume [--out DIR] [--max-urls N] [--concurrency N]
coqui report [summary|issues|status|performance|redirects|broken-links|overview] [--out DIR]
coqui report compare --with <crawl id> [--out DIR] [--top N] [--status CODE]
coqui reports
coqui export [--out DIR]
coqui extractions [--out DIR]
coqui sitemap [--out DIR] [--split] [--gzip]
```

Examples:

```bash
coqui crawl url https://example.com/
coqui crawl sitemap https://example.com/sitemap.xml --max-urls 2000 --out crawls/example
coqui crawl list ./urls.txt --depth 1 --concurrency 4 --ignore-robots
coqui crawl url https://example.com/ --render --screenshots --a11y --vitals --out crawls/rendered
```

## Rendered output

With `--render`, pages are rendered in headless Chrome/Edge before extraction. `--screenshots` writes JPEGs to `DIR/screenshots/<crawl-id>/`, `--a11y` stores axe violations, and `--vitals` stores LCP/CLS/TBT/TTFB/FCP. Verified against the local fixture: rendered title/H1, one screenshot, console errors, axe violations and vitals all captured.

## Config file

Pass `--config PATH` to load defaults from TOML; without the flag, `coqui.toml` in the working directory is used when present. CLI flags override config values.

```toml
out = "crawls/example"
max_urls = 5000
depth = 3
concurrency = 8
respect_robots = true
user_agent = "Coqui/0.1"
headers = ["X-Test: yes"]
auth = "user:pass"
timeout_ms = 20000
render = false
screenshots = false
accessibility = false
web_vitals = false
render_workers = 2
```

## Resume

`coqui resume --out DIR` reloads the latest crawl and its checkpoint from `DIR/coqui.db`, then continues. `--max-urls` and `--concurrency` override the stored config for the resumed run. If no checkpoint exists, the command exits with an error.

## MCP server

`coqui mcp --out DIR` runs a stdio MCP server (JSON-RPC, newline-delimited) over `DIR/coqui.db`. Tools: `list_crawls`, `get_crawl`, `list_urls`, `issue_counts`, `export_urls`, `export_family` (family or `all`), `compare_crawls`, `start_crawl`, `run_schedule_due`. Implements `initialize`, `ping`, `tools/list`, `tools/call`, plus resources (`coqui://crawls`, `coqui://crawl/{id}`, `coqui://crawl/{id}/issues`) with `resources/list`, `resources/templates/list`, `resources/read`, and prompts (`audit-crawl`, `summarize-issues`, `export-crawl`) with `prompts/list`, `prompts/get`. Verified with a piped session (initialize + tools/list + list_urls).

## Issue settings

```bash
coqui issue-settings export --out DIR [--file PATH]
coqui issue-settings import --out DIR --file PATH
coqui issue-settings diff --out DIR --with CRAWL_ID
```

`diff` prints per-rule differences between the latest crawl's settings and another crawl.

Exports a crawl's suppressions and severity overrides as JSON (default `issue-settings.json`) and imports them into the latest crawl, replacing its current settings. Verified with an empty settings file on the validation crawl.

## Prune

`coqui prune --out DIR [--keep N]` deletes all but the N most recent crawls (default 10), including their screenshot files. Verified: pruning a two-crawl database with `--keep 1` removed one crawl; a second run reported nothing to delete.

## Overview

`coqui overview --out DIR` prints the same crawl aggregates as the app overview (issues by severity, statuses, content, indexability, response stats/buckets, depth, segments) and the aggregate timing. On the 1M-row fixture it took 2.7 s cold / 2.0 s warm.

## Serve (HTTP API)

`coqui serve --out DIR [--addr 127.0.0.1:8787]` starts the read-only JSON API (see [api.md](api.md)).

## Doctor

`coqui doctor --out DIR` prints the CLI version and checks database writability, bundled Node, Lighthouse CLI and a detected Chrome/Edge browser. Verified output on this machine:

```
coqui 0.1.0
database: ok (research\validation-79\coqui.db)
node: ok (src-tauri/sidecars/node.exe)
lighthouse: ok (src-tauri/sidecars/lighthouse/node_modules/lighthouse/cli/index.js)
browser: ok (C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe)
```

## Schedules

```bash
coqui schedule add url https://example.com/ --every 3600 --out crawls/example
coqui schedule list --out crawls/example
coqui schedule run-due --out crawls/example
coqui schedule remove 1 --out crawls/example
```

Schedules are interval-based (seconds) and stored in the crawl DB. `run-due` runs every enabled schedule whose `next_run_at` has passed, writes `<crawl>.csv`, and advances the next run. Designed to be called by cron/Task Scheduler/CI. Verified end-to-end: add → run-due (3 rows) → list showed last/next → remove.

## Sitemaps

`coqui sitemap --out DIR` writes `DIR/sitemap.xml` from the latest crawl (200s only, noindex excluded, image entries, lastmod). `--split` writes a 50k-chunk bundle under `DIR/sitemaps` with `sitemap-index.xml`; `--gzip` compresses parts. Verified: 3-URL crawl produced sitemap.xml and a gzipped bundle.

## Custom extraction

`coqui schedule add ... --webhook URL --delay-ms N --max-rps N` stores those in the schedule config so scheduled runs notify and stay within rate limits.

`--delay-ms N` waits at least N milliseconds between HTTP requests; `--max-rps N` caps requests per second per host (effective delay = max of both). `--proxy URL` routes requests through an http/https/socks5 proxy; invalid proxies fail with a configuration error. `--cookie "a=b; c=d"` sends a Cookie header for logged-in crawling (verified with a local echo server).

`--webhook URL` posts a JSON completion payload (`crawl.finished`, crawlId, paused, stats) with a 5-second timeout.

`--extract NAME=SELECTOR` (repeatable) registers CSS extractors before the crawl; values are stored per URL. `coqui extractions --out DIR` exports `url,name,value` rows. Verified: two extractors across three fixture pages produced 6 rows.

## Export

`coqui export --out DIR` streams the latest crawl's URLs to `DIR/urls-<timestamp>.csv` using the row-by-row exporter. 10,000 rows exported in 0.009 s against a local 10k crawl.

## Reports

`coqui report <kind> --pdf` also writes a PDF via headless Chrome/Edge (absolute output path, `COQUI_BROWSER_PATH` override). Verified: summary report produced a 71 KB PDF.

`coqui reports` lists report types; `coqui report issues --out DIR` renders an HTML report from the latest crawl in `DIR/coqui.db` to `DIR/report-issues.html`. `--top N` limits list rows and `--status CODE` filters by HTTP status. Verified end-to-end (`report performance --top 1 --status 200`).

## Output

- `DIR/coqui.db` — portable SQLite crawl file.
- `DIR/urls.csv` — flat URL export with the default columns.
- Console summary: crawled, discovered, total issues, elapsed time.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Crawl completed |
| 1 | Crawl failed |
| 2 | Invalid arguments |

## Current limits

Supports URL/sitemap/list seeds, crawl limits, user agent, custom headers, basic auth, timeout, and rendering with screenshots/axe/Web Vitals. Export formats, config files, profiles and checkpoint/resume flags are next.















## Custom JavaScript

`--js EXPR` (repeatable) evaluates JavaScript in each rendered page after load; requires `--render`. Results are stored per URL and exported as Custom JS results (`custom-js`) in the app. Verified: `--js "document.title"` returned "Rendered title" on the JS fixture.
























