# Rendering (M4 foundation)

Coqui can render pages in an external headless Chrome/Edge before extraction. This is the first slice of the rendering milestone: JavaScript-dependent DOM, not yet screenshots, Lighthouse or axe.

## How it works

- `coqui-render` speaks the Chrome DevTools Protocol over a local WebSocket.
- It launches a headless browser on a random debugging port with a throwaway profile, or connects to `COQUI_CDP_ENDPOINT` if set.
- A `RenderPool` runs up to 1–8 tabs in parallel on a **single** browser (default 2 tabs). One multiplexed CDP connection correlates concurrent calls by id and broadcasts events to each tab.
- For each page it creates a tab, navigates, polls `document.readyState` until `complete`, waits `render_wait_ms`, reads `document.documentElement.outerHTML`, and closes the tab.
- Each worker is serialised; the pool renders `render_workers` pages concurrently. Raw fetches still run concurrently around it.
- `coqui-crawl` fetches the page normally for status/headers/blocking, then replaces the HTML body with the rendered DOM when `render` is enabled.

## Configuration

| Field | Meaning |
| --- | --- |
| `render` | Render pages before extraction |
| `browserPath` | Optional explicit Chrome/Edge executable |
| `renderWaitMs` | Extra delay after load (default 500 ms) |
| `renderWorkers` | Number of browser workers (1–8, default 2) |
| `screenshots` | Capture a JPEG screenshot per rendered page |
| `renderBlockAssets` | Block fonts/media (default off; no measured gain) |
| `renderBlockPatterns` | Extra URL patterns blocked while rendering (JSON/harness) |
| `accessibility` | Run axe-core on each rendered page |
| `webVitals` | Capture LCP, CLS, TBT, TTFB, FCP |
| `renderEarly` | Continue at DOMContentLoaded instead of full load (default off) |

Environment overrides for the CLI/harness: `COQUI_BROWSER_PATH`, `COQUI_CDP_ENDPOINT`, `COQUI_RENDER=1`, `COQUI_RENDER_WAIT`, `COQUI_RENDER_WORKERS`, `COQUI_SCREENSHOTS=1`.

Auto-detection order: `COQUI_BROWSER_PATH`, configured path, then Edge/Chrome in their standard install locations.

## Rendered-page evidence

With `screenshots` enabled, each rendered page also captures:

- a JPEG screenshot (`Page.captureScreenshot`, quality 60) stored in the `screenshots` table keyed by crawl and URL id;
- browser console errors and uncaught exceptions, collected from CDP `Runtime.consoleAPICalled`, `Runtime.exceptionThrown` and `Log.entryAdded` events and stored as `urls.console_json`, prefixed `console:`, `js:` and `network:` so JS exceptions can be separated from resource-load failures.

The app exposes `get_screenshot(crawlId, urlId)` returning a data URL, shows a `JS n` badge on rows with console errors, and opens screenshots in a modal.

## Authenticated rendering

Custom headers, cookies and the configured user agent are pushed into each browser session before navigation:

- `Cookie` headers are split into `Network.setCookie` calls for the page URL.
- Other headers go through `Network.setExtraHTTPHeaders`; basic auth becomes an `Authorization` header.
- User agent is applied with `Emulation.setUserAgentOverride`.

## Web Vitals

With `webVitals` enabled, each rendered tab reports LCP, CLS, TBT, TTFB and FCP. The renderer forces a 1×1 paint first (headless pages otherwise skip paint timing), waits briefly for paint entries, then reads buffered PerformanceObserver data and navigation timing.

Values are stored per URL as `vitals_json`; the `poor_lcp`, `layout_shift`, `slow_ttfb` and `long_blocking_time` rules surface them, and LCP/CLS are available as grid columns.

Staging, 50 pages: median LCP 2,964 ms, median TTFB 1,979 ms, 37 poor LCP, 49 slow TTFB, no layout shift or long-blocking. Capturing vitals added no measurable crawl time (63.9 s vs 64.1 s without).

## Accessibility (axe)

With `accessibility` enabled, each rendered tab also gets axe-core (vendored 4.10.2, MPL-2.0) injected and run against the document. Violations are stored per URL as `axe_json` and surfaced through the `accessibility_violations` audit rule.

Stored fields per violation: rule id, impact, help text, help URL, node count. A 50-page staging run found 315 nodes across 5 rules — mostly `color-contrast` (199) and `region` (100) — and added ~13% to render time.

## Performance

Measured on `stage.landscapemanagement.net` (heavy third-party pages):

| Scenario | 50 pages | Processes |
| --- | --- | --- |
| Four separate browsers (old) | 85–93 s | ~144 |
| One browser, 4 tabs | 64.1 s (~0.8/s) | 34 |
| One browser, 8 tabs | 63.1 s (~0.8/s) | 32 |
| 8 tabs + third-party blocking | 65.0 s | ~32 |

A single stage homepage took **14.6 s** to reach `readyState=complete`. Sharing one browser cut render time by ~30% and OS processes by ~4×; adding tabs beyond four and blocking third-party scripts did not move the needle further. Remaining throughput is bounded by the site's own load time.

## Limits

- A worker renders one page at a time; the pool size is the concurrency limit.
- No request interception or response body capture yet.
- No screenshots, console/network capture, Lighthouse or axe.
- Pages that never reach `readyState=complete` fail after 20 seconds and fall back to the raw HTML.

## Verification

- `cargo test -p coqui-render -- --ignored` runs the live browser test (requires Chrome/Edge).
- `node tools/fixture/site.mjs 4174`, then `COQUI_RENDER=1 cargo run -p coqui-crawl --example audit -- url http://127.0.0.1:4174/js out 1 1` should record the JS-generated title and H1.
- Live check: `cargo run -p coqui-render --example render -- https://www.landscapemanagement.net/` returned ~274 KB of rendered HTML.



