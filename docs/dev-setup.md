# Local development

## Prerequisites

| Requirement | Notes |
| --- | --- |
| Rust 1.90+ (MSVC toolchain on Windows) | `rustup default stable`; the workspace uses edition 2024 |
| Node.js 22+ and npm 10+ | Next.js 16 and the Tauri CLI |
| Tauri system dependencies | Windows: WebView2 (preinstalled on Win 11) + VS Build Tools with the C++ workload. macOS: Xcode command line tools. Linux: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf` |
| Chrome or Edge | Needed for rendering, axe, Web Vitals and Lighthouse. Coqui discovers a system browser, or install Chrome for Testing with `coqui browser install` |

## Install

```bash
npm install          # root workspace + apps/web
```

## Run the desktop app

```bash
npm run dev          # Tauri dev: starts Next on :3000, then opens the native window
```

The window is the real app: Rust commands run in-process, results are stored in SQLite, and rendering uses the detected browser.

## Browser-only preview (no crawling)

```bash
npm run dev:web      # http://localhost:3000
```

The UI renders, but crawl controls are disabled because there is no Tauri IPC bridge. Use this for layout and styling work.

## Test and check

```bash
cargo fmt --all --check
cargo test --workspace        # 112 pass; 5 render tests are ignored without a browser
cargo test -p coqui-render -- --ignored   # requires Chrome/Edge
npm run build:web             # verifies the static export used by Tauri
node tools/docs/check.mjs     # docs match code counts, commands, links and report kinds
```

The crawler tests spin up a local HTTP fixture site, so they need no network access.

## Fixtures and audits

```bash
npm run fixture        # 5-URL fixture on :4173
npm run fixture:bulk   # 5,000-URL synthetic site on :4180
npm run audit -- sitemap http://127.0.0.1:4180/sitemap.xml ./bulk 10000 16
#                 kind    seed                          dir   max  concurrency
```

The audit example crawls one site and prints throughput, status mix, coverage and issues.

## Benchmarks and parity

```bash
cargo run -p coqui-cli -- bench --urls 100000 --out bench-out --json
node tools/bench/check.mjs --input bench.json
node tools/parity/compare-exports.mjs \
  --reference tools/parity/fixtures/reference \
  --coqui tools/parity/fixtures/coqui \
  --tolerance tools/parity/fixtures/tolerance.json \
  --require-counterparts --max-mismatches 0
```

Parity reruns against the real Screaming Frog export need the local `research/` artifacts described in [handover.md](handover.md).

## Sidecars and browser

```bash
npm run sidecars          # pinned Node + Lighthouse into src-tauri/sidecars (gitignored)
coqui browser install     # Chrome for Testing into the OS cache
coqui browser path        # print the executable Coqui would use
coqui doctor              # database, Node, Lighthouse and browser checks
```

## Build installers

```bash
npm run build        # static export + cargo build --release + bundle for the current OS
```

Artifacts land in `src-tauri/target/release/bundle/`.

## Where data lives

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\app.coqui.desktop\` |
| macOS | `~/Library/Application Support/app.coqui.desktop/` |
| Linux | `~/.local/share/app.coqui.desktop/` |

Inside: `coqui.db` (all crawls), `exports/` (CSV output).

## Repository map

```
apps/web/          Next.js 16 static-export UI
crates/coqui-core/ Domain types, config, URL identity
crates/coqui-store/SQLite schema (v35) and paged queries
crates/coqui-crawl/HTTP frontier, extraction, crawl engine
crates/coqui-audit/Rules, suppressions and overrides
crates/coqui-render/Chromium/CDP, screenshots, axe, vitals
crates/coqui-report/HTML/PDF reports and comparison
crates/coqui-api/  HTTP API (axum)
crates/coqui-cli/  Headless CLI and MCP server
src-tauri/         Tauri shell: commands, state, events
docs/              Inventory, plan, evidence, references, validation
tools/plan/        Backlog source and generators
tools/parity/      Reference import/compare harness and fixtures
tools/bench/       Benchmark budgets and regression comparison
tools/docs/       Documentation consistency check
```

## Current state (M6 — parity breadth and release engineering)

Implemented: crawling (URL/sitemap/list seeds, robots, redirects, auth, headers, cookies, proxy, pacing, checkpoints, profiles, webhooks), Chromium rendering (screenshots, console, axe, vitals, custom JS, authenticated rendering), **50 audit rules plus enumerated `axe_*`**, saved views, crawl history, comparison, profiles, schedules, Lighthouse, 15 export families, 15 report kinds, HTTP API, MCP server, benchmark and parity harnesses, release packaging and self-update.

Not yet implemented: third-party integrations (Search Console, GA4, PSI, Ahrefs, Majestic, Moz), AI features, the reference product's 34-tab workspace, locales, Postgres/server mode, projects/workspace lifecycle, SERP and scripted-login modes, bundled Chromium, code-signed installers.

## Troubleshooting

- **`link.exe not found`** — install the Visual Studio C++ build tools and reopen the terminal.
- **Blank window in `npm run dev`** — the Next dev server must be reachable at `http://localhost:3000`; check the Tauri terminal output.
- **Render fails with “no browser”** — run `coqui browser install` or set `COQUI_BROWSER_PATH` to Chrome/Edge.
- **Lighthouse reports missing** — run `npm run sidecars`, or install Node 20+ and let it fall back to `npx`.
