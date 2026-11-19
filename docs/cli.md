# Coqui CLI

Headless crawler using the same engine, store and audit rules as the desktop app. First cut: `coqui crawl`.

## Usage

```bash
coqui crawl <url|sitemap|list> <seed> [--out DIR] [--max-urls N] [--depth N] [--concurrency N] [--ignore-robots] [--user-agent UA] [--header "Name: value"] [--auth user:pass] [--timeout-ms N] [--render] [--screenshots] [--a11y] [--vitals] [--render-workers N]
```

Examples:

```bash
coqui crawl url https://example.com/
coqui crawl sitemap https://example.com/sitemap.xml --max-urls 2000 --out crawls/example
coqui crawl list ./urls.txt --depth 1 --concurrency 4 --ignore-robots
```

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

