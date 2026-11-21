# Coqui CLI

Headless crawler and automation surface. It uses the same engine, store, audit rules and report/export code as the desktop app. Every command reads or writes a portable SQLite crawl file (`DIR/coqui.db`).

```bash
coqui <command> [args]
coqui --version
```

The default output directory is `coqui-out`; pass `--out DIR` to change it.

## Commands

| Command | Purpose |
| --- | --- |
| `crawl` | Crawl a URL, sitemap or URL list |
| `resume` | Continue the latest crawl from its checkpoint |
| `report` | Render an HTML/PDF report, or compare two crawls |
| `reports` | List report kinds |
| `export` | Export a crawl to CSV families |
| `exports` | List the app export catalogue (id, format, label, description) |
| `extractions` | Export custom extraction values |
| `sitemap` | Generate XML/image sitemaps |
| `schedule` | Add, list, remove or run due interval schedules |
| `serve` | Start the HTTP API |
| `overview` | Print crawl aggregates |
| `prune` | Delete all but the N most recent crawls |
| `doctor` | Check database, Node, Lighthouse and browser |
| `bench` | Storage/query benchmark |
| `browser` | Install or locate a Chrome for Testing build |
| `update` | Check for or install a newer release |
| `import sf` | Import a Screaming Frog `.dbseospider` |
| `issue-settings` | Export/import/diff suppressions and overrides |
| `mcp` | Run the stdio MCP server |

## crawl

```bash
coqui crawl <url|sitemap|list> <seed> [options]
```

Examples:

```bash
coqui crawl url https://example.com/
coqui crawl sitemap https://example.com/sitemap.xml --max-urls 2000 --out crawls/example
coqui crawl list ./urls.txt --depth 1 --concurrency 4 --ignore-robots
coqui crawl url https://example.com/ --render --screenshots --a11y --vitals --out crawls/rendered
```

| Flag | Default | Meaning |
| --- | --- | --- |
| `--out DIR` | `coqui-out` | Crawl directory (`DIR/coqui.db`, `DIR/urls.csv`) |
| `--max-urls N` | 5000 | Hard cap on fetched URLs |
| `--depth N` | 3 | Maximum link depth from the seed |
| `--concurrency N` | 8 | Concurrent HTTP requests |
| `--ignore-robots` | off | Do not fetch or obey robots.txt |
| `--user-agent UA` | Coqui UA | Override the User-Agent |
| `--header "Name: value"` | — | Extra request header (repeatable) |
| `--auth user:pass` | — | HTTP basic authentication |
| `--cookie "a=b; c=d"` | — | Cookie header for logged-in crawling |
| `--timeout-ms N` | 20000 | Per-request timeout |
| `--delay-ms N` | 0 | Minimum delay between requests |
| `--max-rps N` | 0 | Per-host requests-per-second cap (0 = unlimited) |
| `--proxy URL` | — | HTTP/HTTPS/SOCKS5 proxy |
| `--render` | off | Render pages in Chromium before extraction |
| `--render-workers N` | 2 | Concurrent rendering tabs |
| `--render-wait-selector CSS` | — | Wait for this selector after load |
| `--screenshots` | off | Save a JPEG per rendered page |
| `--a11y` | off | Run axe-core (requires `--render`) |
| `--vitals` | off | Capture LCP/CLS/TBT/TTFB/FCP (requires `--render`) |
| `--js EXPR` | — | Evaluate JavaScript per rendered page (repeatable) |
| `--extract NAME=SELECTOR` | — | Register a CSS extraction (repeatable) |
| `--store-text` | off | Store visible text and Markdown per URL |
| `--store-html` | off | Store raw HTML per URL |
| `--webhook URL` | — | POST a completion payload |
| `--json` | off | One machine-readable summary line |
| `--ndjson` | off | One JSON object per progress event, plus a final event |
| `--fail-on-issues N` | — | Exit 3 when the crawl produces more than N issues |
| `--config PATH` | `coqui.toml` | Load defaults from TOML |

### Config file

`--config PATH` loads defaults from TOML; without the flag, `coqui.toml` in the working directory is used when present. CLI flags override config values.

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

### Output

- `DIR/coqui.db` — portable SQLite crawl file (schema v35).
- `DIR/urls.csv` — flat URL export with the default columns.
- Console summary: crawled, discovered, total issues and elapsed time.
- With `--json`: one object with `status`, `crawlId`, `crawled`, `discovered`, `issues`, `seconds` and `csv`.
- With `--ndjson`: progress events carry `crawlId` and `ts`, followed by a `completed` or `failed` event.

### Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Crawl completed within the issue threshold |
| 1 | Crawl failed |
| 2 | Invalid arguments or configuration |
| 3 | `--fail-on-issues` threshold exceeded |

## resume

```bash
coqui resume --out DIR [--max-urls N] [--concurrency N]
```

Reloads the latest crawl and its checkpoint from `DIR/coqui.db` and continues. `--max-urls` and `--concurrency` override the stored config. If no checkpoint exists the command exits with an error.

## export

```bash
coqui export --out DIR [--family FAMILY]
```

Families: `urls`, `issues`, `links`, `headers`, `images`, `a11y`, `structured`, `vitals`, `console`, `text`, `markdown`, `extractions`, `custom-js`, `anchors`, `social`, or `all` for the 15 families in one pass.

- The default family is `urls`.
- Files are named `DIR/<family>-<milliseconds>.csv`; `all` prints one line per family and a total.
- `text` and `markdown` need `--store-text`; `custom-js` needs `--js` with `--render`; `vitals`/`a11y` need `--vitals`/`--a11y`.
- The app also offers XLSX, a per-view export catalogue, filename tokens and skip-empty; those are Tauri export features, not CLI flags. See [exports.md](exports.md).

## report / reports

```bash
coqui report <kind> --out DIR [--top N] [--status CODE] [--title TEXT] [--pdf]
coqui report all --out DIR
coqui report compare --with <crawl id> --out DIR [--top N] [--status CODE]
coqui reports
```

Kinds: `summary`, `issues`, `status`, `performance`, `redirects`, `broken-links`, `overview`, `hreflang`, `canonical-chains`, `pagination`, `orphans`, `segments`, `serp`, `canonicals`, `hreflang-return`. See [reports.md](reports.md) for contents.

- Output is `DIR/report-<kind>.html`, plus `.pdf` with `--pdf` (headless Chrome/Edge; `COQUI_BROWSER_PATH` overrides discovery).
- `report all` renders every kind, isolates failures and prints written/failed counts.
- `report compare` writes `report-compare.html`, `compare.csv` and `compare-urls.csv` against another crawl id in the same database.
- `reports` lists kinds and descriptions.

## exports

```bash
coqui exports
```

Prints the 16-entry export catalogue as tab-separated `id`, `format`, `label` and `description`; the desktop app uses the same catalogue.

## extractions

```bash
coqui extractions --out DIR
```

Exports `url,name,value` rows captured by `crawl --extract`.

## sitemap

```bash
coqui sitemap --out DIR [--split] [--gzip] [--images]
```

Writes `DIR/sitemap.xml` from the latest crawl (2xx, noindex excluded) with `lastmod` when the crawl stored a `Last-Modified` header. `--split` writes a 50,000-URL bundle under `DIR/sitemaps` with `sitemap-index.xml`; `--gzip` compresses parts; `--images` includes image entries.

## schedule

```bash
coqui schedule add <kind> <seed> --every SECONDS [--out DIR] [--max-urls N] [--delay-ms N] [--max-rps N] [--proxy URL] [--webhook URL]
coqui schedule list --out DIR
coqui schedule remove <id> --out DIR
coqui schedule run-due --out DIR
```

Schedules are interval-based (seconds) and stored in the crawl database. `run-due` runs every enabled schedule whose `next_run_at` has passed, writes the crawl CSV and advances the next run. Designed for cron, Task Scheduler or CI.

## serve

```bash
coqui serve --out DIR [--addr 127.0.0.1:8787] [--token TOKEN]
```

Starts the HTTP API. Loopback is the default; set `--token` or `COQUI_API_TOKEN` to require `Authorization: Bearer <token>` on every route except `/health`. See [api.md](api.md).

## overview

```bash
coqui overview --out DIR
```

Prints issues by severity, statuses, content, indexability, response stats/buckets, depth, segments, aggregate timing and a per-second throughput summary.

## prune

```bash
coqui prune --out DIR [--keep N]
```

Deletes all but the N most recent crawls (default 10), including screenshot files. Refuses while a crawl is running.

## doctor

```bash
coqui doctor --out DIR
```

Prints the CLI version and checks database writability, Node, the Lighthouse CLI and a detected Chrome/Edge browser. Example on macOS:

```
coqui 0.1.0
database: ok (/path/to/coqui.db)
node: ok (v24.22.2)
lighthouse: missing (run `npm run sidecars`)
browser: ok (/Applications/Google Chrome.app/Contents/MacOS/Google Chrome)
```

## bench

```bash
coqui bench [--urls N] [--chunk N] [--out DIR] [--json] [--force]
```

Writes a synthetic crawl to a throwaway database and reports insert throughput, `all_urls` query time, page-query p95 and bytes/URL. Refuses to overwrite an existing `coqui.db` unless `--force` is passed. See [benchmarks.md](benchmarks.md).

## browser

```bash
coqui browser install [--out DIR] [--sha256 HEX] [--dry-run]
coqui browser path
```

Downloads the current stable Chrome for Testing for the platform, verifies SHA-256 when `--sha256` is supplied, extracts it under the OS cache and prints the executable path. `path` prints what would be used. `COQUI_BROWSER_PATH` overrides discovery everywhere.

## update

```bash
coqui update [--json]
coqui update --install
```

Compares the running version with the latest GitHub release. `--json` prints `current`, `latest`, `update_available` and `url`. `--install` downloads the matching OS/arch binary, verifies it against `SHA256SUMS-<os>-<arch>.txt` and replaces the executable; it cannot be combined with `--json`. No release is published yet, so this currently prints `no releases published yet`.

## import sf

```bash
coqui import sf --file path/to/crawl.dbseospider --out DIR [--pages-only]
```

Screaming Frog's `.dbseospider` is a ZIP whose `results_*/sql` member is a SQLite database. Coqui imports internal URLs (title, meta, H1, status, word count, depth, response time, size) and anchor rows for internal/external links. `--pages-only` skips resource rows. The imported crawl is a normal Coqui crawl; audit rules are not run automatically. Verified against a 267-URL reference crawl (211 URLs and 3,826 links imported).

## issue-settings

```bash
coqui issue-settings export --out DIR [--file PATH]
coqui issue-settings import --out DIR --file PATH
coqui issue-settings diff --out DIR --with CRAWL_ID
```

Exports a crawl's suppressions and severity overrides as JSON (default `issue-settings.json`) and imports them into the latest crawl, replacing its current settings. `diff` prints per-rule differences against another crawl.

## mcp

```bash
coqui mcp --out DIR
```

Runs a stdio MCP server (JSON-RPC, newline-delimited) over `DIR/coqui.db`.

- **Tools:** `list_crawls`, `get_crawl`, `list_urls`, `issue_counts`, `export_urls`, `export_family`, `compare_crawls`, `start_crawl`, `run_schedule_due`.
- **Resources:** `coqui://crawls`, `coqui://crawl/{id}`, `coqui://crawl/{id}/issues` with list/template/read.
- **Prompts:** `audit-crawl`, `summarize-issues`, `export-crawl`.
- Implements `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/templates/list`, `resources/read`, `prompts/list` and `prompts/get`.

## Current limits

- No `profile` command: crawl profiles are a desktop-app feature.
- No projects/workspaces, Postgres or multi-user mode; one SQLite file is one crawl store.
- No Scheduler daemon: `schedule run-due` must be invoked by cron, Task Scheduler or CI.
- No integrations, AI commands or SERP mode.
- Report/export kinds and audit rules cover a subset of the reference product; see [status.md](status.md).
