# Rendering

Coqui renders pages in a headless Chromium/Chrome over the Chrome DevTools Protocol (CDP) before extraction. Rendering is opt-in (`--render` in the CLI, the render toggle in the app) and adds screenshots, console diagnostics, axe accessibility, Web Vitals and network metrics.

## How it works

- `coqui-render` speaks CDP over a local WebSocket. It launches a headless browser on a random debugging port with a throwaway profile, or connects to `COQUI_CDP_ENDPOINT` when set.
- A `RenderPool` runs up to eight tabs on a **single** browser (default 2). One multiplexed CDP connection correlates concurrent calls by id and routes events to each tab.
- For each page it creates a tab, navigates, waits for `readyState=complete` (or `DOMContentLoaded` with early mode), applies `render_wait_ms`, optionally waits for a CSS selector, reads `document.documentElement.outerHTML`, captures evidence, and closes the tab.
- `coqui-crawl` fetches the page normally for status/headers, then replaces the HTML body with the rendered DOM before extraction.
- Discovery order: `COQUI_BROWSER_PATH`, the configured browser path, the Chrome for Testing cache installed by `coqui browser install`, then Edge/Chrome in standard install locations.

## Configuration

| Field | CLI | Meaning |
| --- | --- | --- |
| `render` | `--render` | Render before extraction |
| `browserPath` | `COQUI_BROWSER_PATH` | Explicit Chrome/Edge executable |
| `renderWaitMs` | config only | Extra delay after load (default 500 ms) |
| `renderWorkers` | `--render-workers` | Concurrent tabs (1–8, default 2) |
| `screenshots` | `--screenshots` | JPEG screenshot per rendered page |
| `renderBlockAssets` | config only | Block fonts/media (default off) |
| `renderBlockPatterns` | config only | Extra URL patterns blocked while rendering |
| `accessibility` | `--a11y` | Run axe-core per page |
| `webVitals` | `--vitals` | Capture LCP, CLS, TBT, TTFB, FCP |
| `renderEarly` | config only | Continue at DOMContentLoaded (default off) |
| `renderWaitSelector` | `--render-wait-selector` | Wait for a CSS selector after load |
| `customJs` | `--js` | JavaScript evaluated per page after load |

Environment overrides for the CLI/harness: `COQUI_BROWSER_PATH`, `COQUI_CDP_ENDPOINT`, `COQUI_RENDER=1`, `COQUI_RENDER_WAIT`, `COQUI_RENDER_WORKERS`, `COQUI_SCREENSHOTS=1`.

`Accept-Language` is not a dedicated field; pass it as a header (`--header "Accept-Language: de-DE"`, or the app's field, which adds the same header).

## Screenshots and console

- Screenshots use `Page.captureScreenshot` (JPEG, quality 60) and are written to `<crawl-dir>/screenshots/<crawl-id>/<url-id>.jpg`; the database stores only the relative path in `screenshot_files`.
- Console errors and uncaught exceptions are collected from `Runtime.consoleAPICalled`, `Runtime.exceptionThrown` and `Log.entryAdded`, stored as `urls.console_json` and prefixed `console:`, `js:` or `network:` so exceptions can be separated from resource failures.
- The app exposes `get_screenshot(crawlId, urlId)` as a data URL, shows a `JS n` badge on rows with console errors, and opens screenshots in a modal.

## Authenticated rendering

Custom headers, cookies and the configured user agent are pushed into each browser session before navigation:

- `Cookie` headers become `Network.setCookie` calls for the page URL.
- Other headers go through `Network.setExtraHTTPHeaders`; basic auth becomes an `Authorization` header.
- The user agent is applied with `Emulation.setUserAgentOverride`.

## Web Vitals

Each rendered tab reports LCP, CLS, TBT, TTFB and FCP. The renderer forces a 1×1 paint first (headless pages otherwise skip paint timing), waits briefly for paint entries, then reads buffered PerformanceObserver data and navigation timing.

Values are stored per URL as `vitals_json`; `poor_lcp`, `layout_shift`, `slow_ttfb`, `long_blocking_time`, `dom_size_large`, `render_blocking_requests`, `avoid_enormous_network_payload`, `improve_image_delivery` and `viewport_not_set` surface them. LCP/CLS are grid columns.

Staging, 50 pages: median LCP 2,964 ms, median TTFB 1,979 ms, 37 poor LCP, 49 slow TTFB, no layout shift or long-blocking. Capturing vitals added no measurable crawl time (63.9 s vs 64.1 s).

## Accessibility (axe)

With accessibility enabled, each tab gets axe-core (vendored 4.10.2, MPL-2.0) injected and run against the document. Violations are stored as `axe_json`, and the audit emits **one issue per violation** (`axe_<id>`) rather than the legacy aggregate `accessibility_violations` rule:

- Severity from axe impact: `critical`/`serious` → Error, `moderate` → Warning, `minor` → Opportunity.
- Detail is the node count plus axe help text.

A 50-page staging run found 315 nodes across 5 rules — mostly `color-contrast` (199) and `region` (100) — and added about 13% to render time. The rendered validation on adamgreenwell.com stored vitals for 20/20 pages and produced enumerated axe rules including `axe_color_contrast`, `axe_heading_order` and `axe_label`.

## Performance

Measured on `stage.landscapemanagement.net` (heavy third-party pages):

| Scenario | 50 pages | Processes |
| --- | --- | --- |
| Four separate browsers (old) | 85–93 s | ~144 |
| One browser, 4 tabs | 64.1 s (~0.8/s) | 34 |
| One browser, 8 tabs | 63.1 s (~0.8/s) | 32 |
| 8 tabs + third-party blocking | 65.0 s | ~32 |

A single stage homepage took 14.6 s to reach `readyState=complete`. Sharing one browser cut render time by ~30% and OS processes by ~4×; more tabs and third-party blocking did not move the needle further. Throughput is bounded by the site's own load time.

## Limits

- A worker renders one page at a time; the pool size is the concurrency limit.
- No request interception or response-body capture yet; the rendered DOM and network metrics are the evidence.
- A page that never reaches the wait condition fails after 20 seconds and falls back to the raw HTML.
- Lighthouse launches its own browser in a separate process; crawl cookies/headers are not applied to it.

## Verification

- `cargo test -p coqui-render -- --ignored` runs the live browser tests (Chrome/Edge required).
- `node tools/fixture/site.mjs 4174`, then `COQUI_RENDER=1 cargo run -p coqui-crawl --example audit -- url http://127.0.0.1:4174/js out 1 1` records the JS-generated title and H1.
- Live check: `cargo run -p coqui-render --example render -- https://www.landscapemanagement.net/` returned ~274 KB of rendered HTML.
