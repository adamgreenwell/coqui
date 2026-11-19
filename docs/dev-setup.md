# Local development

## Prerequisites

| Requirement | Notes |
| --- | --- |
| Rust 1.90+ (MSVC toolchain on Windows) | `rustup default stable`; the workspace uses edition 2024 |
| Node.js 22+ and npm 10+ | Next.js 16 and the Tauri CLI |
| Tauri system dependencies | Windows: WebView2 (preinstalled on Win 11) + VS Build Tools with the C++ workload. macOS: Xcode command line tools. Linux: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf` |
| Chromium (later phases) | Only needed when rendering lands; Coqui will discover a system browser or a pinned download |

## Install

```bash
npm install          # root workspace + apps/web
```

## Run the desktop app

```bash
npm run dev          # Tauri dev: starts Next on :3000, then opens the native window
```

The window is the real app: Rust commands run in-process, results are stored in SQLite.

## Browser-only preview (no crawling)

```bash
npm run dev:web      # http://localhost:3000
```

The UI renders, but crawl controls are disabled because there is no Tauri IPC bridge. Use this for layout and styling work.

## Test and check

```bash
cargo test -p coqui-core -p coqui-store -p coqui-crawl
cargo check --workspace
npm run build:web    # verifies the static export used by Tauri
```

The crawler tests spin up a local HTTP fixture site, so they need no network access.

## Build installers

```bash
npm run build        # static export + cargo build --release + bundle for the current OS
```

Artifacts land in `src-tauri/target/release/bundle/`.

## Manual validation harness

The audit example crawls one site and prints throughput, status mix, coverage and issues:

```bash
npm run audit -- sitemap https://example.com/sitemap.xml ./out 2000 4
#               kind    seed                          dir   max  concurrency
```

For scale testing without external load:

```bash
npm run fixture:bulk   # 5,000-URL synthetic site on :4180
npm run audit -- sitemap http://127.0.0.1:4180/sitemap.xml ./bulk 10000 16
```

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
crates/coqui-store/SQLite schema and paged queries
crates/coqui-crawl/HTTP frontier, extraction, crawl engine
src-tauri/         Tauri shell: commands, state, events
docs/              Inventory, plan, evidence, reference data
tools/plan/        Backlog source and generators
tools/github/      Backlog publisher
```

## Current slice (M6)

Implemented: BFS crawling with robots handling and redirect chains, auth/custom headers, regex scope, URL/sitemap/list seeds, Chromium rendering (screenshots, console, axe, vitals, custom JS), custom extraction, 39 audit rules with saved views and issue filtering, Lighthouse, exports/reports, scheduler and MCP server, SQLite schema v25.

Not yet implemented: cloud/team sync, third-party integrations (Search Console, GA4, Ahrefs, Majestic, Moz), AI features, code-signed installers for every platform, and virtualised 1M-row UI.

## Troubleshooting

- **`link.exe not found`** — install the Visual Studio C++ build tools and reopen the terminal.
- **Blank window in `npm run dev`** — the Next dev server must be reachable at `http://localhost:3000`; check the Tauri terminal output.
- **Port 3000 in use** — stop the other process or run the web app on another port and update `devUrl` in `src-tauri/tauri.conf.json`.
- **Linux WebKit errors** — install the GTK/WebKit packages listed above; WebKitGTK is the most version-sensitive target.












