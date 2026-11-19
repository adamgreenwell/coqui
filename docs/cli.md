# Coqui CLI

Headless crawler using the same engine, store and audit rules as the desktop app. First cut: `coqui crawl`.

## Usage

```bash
coqui crawl <url|sitemap|list> <seed> [--out DIR] [--max-urls N] [--depth N] [--concurrency N] [--ignore-robots] [--user-agent UA] [--header "Name: value"] [--auth user:pass] [--timeout-ms N] [--render] [--screenshots] [--a11y] [--vitals] [--render-workers N]
coqui resume [--out DIR] [--max-urls N] [--concurrency N]
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




