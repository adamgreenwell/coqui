# Rendering (M4 foundation)

Coqui can render pages in an external headless Chrome/Edge before extraction. This is the first slice of the rendering milestone: JavaScript-dependent DOM, not yet screenshots, Lighthouse or axe.

## How it works

- `coqui-render` speaks the Chrome DevTools Protocol over a local WebSocket.
- It launches a headless browser on a random debugging port with a throwaway profile, or connects to `COQUI_CDP_ENDPOINT` if set.
- A `RenderPool` runs several browser workers in parallel (1–8, default 2); each worker renders one page at a time.
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

Environment overrides for the CLI/harness: `COQUI_BROWSER_PATH`, `COQUI_CDP_ENDPOINT`, `COQUI_RENDER=1`, `COQUI_RENDER_WAIT`, `COQUI_RENDER_WORKERS`, `COQUI_SCREENSHOTS=1`.

Auto-detection order: `COQUI_BROWSER_PATH`, configured path, then Edge/Chrome in their standard install locations.

## Rendered-page evidence

With `screenshots` enabled, each rendered page also captures:

- a JPEG screenshot (`Page.captureScreenshot`, quality 60) stored in the `screenshots` table keyed by crawl and URL id;
- browser console errors and uncaught exceptions, collected from CDP `Runtime.consoleAPICalled`, `Runtime.exceptionThrown` and `Log.entryAdded` events and stored as `urls.console_json`.

The app exposes `get_screenshot(crawlId, urlId)` returning a data URL, shows a `JS n` badge on rows with console errors, and opens screenshots in a modal.

## Authenticated rendering

Custom headers, cookies and the configured user agent are pushed into each browser session before navigation:

- `Cookie` headers are split into `Network.setCookie` calls for the page URL.
- Other headers go through `Network.setExtraHTTPHeaders`; basic auth becomes an `Authorization` header.
- User agent is applied with `Emulation.setUserAgentOverride`.

## Limits

- A worker renders one page at a time; the pool size is the concurrency limit.
- No request interception or response body capture yet.
- No screenshots, console/network capture, Lighthouse or axe.
- Pages that never reach `readyState=complete` fail after 20 seconds and fall back to the raw HTML.

## Verification

- `cargo test -p coqui-render -- --ignored` runs the live browser test (requires Chrome/Edge).
- `node tools/fixture/site.mjs 4174`, then `COQUI_RENDER=1 cargo run -p coqui-crawl --example audit -- url http://127.0.0.1:4174/js out 1 1` should record the JS-generated title and H1.
- Live check: `cargo run -p coqui-render --example render -- https://www.landscapemanagement.net/` returned ~274 KB of rendered HTML.


