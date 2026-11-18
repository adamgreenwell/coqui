# ADR-0001: Tauri 2 is the application shell

- **Status:** Accepted
- **Date:** 2026-11-18
- **Context:** Coqui needs to ship on Windows, macOS and Linux from one codebase, crawl at desktop scale, and still expose a CLI and (later) a server/MCP surface.

## Decision

Build the desktop application with **Tauri 2**. The Rust workspace owns crawling, storage, extraction and analysis. The UI is a **Next.js 16 static export** (`output: 'export'`) rendered in the system WebView and talking to Rust through Tauri commands and events. The same crates are reused by `coqui-cli`, a future `coqui-api` (axum) and `coqui-mcp`.

## Why

| Option | Assessment |
| --- | --- |
| **Tauri 2 (chosen)** | One Rust core shared by every surface; small installers; native filesystem/process access for crawl files and Chromium sidecars; typed IPC; system WebView per OS |
| Electron + Rust sidecar | Two runtimes to ship and secure; larger binaries; IPC across processes; more duplication |
| Browser app + local server | No native window, but simplest UI; still needs a server process, and cross-platform distribution becomes "install Docker/Node" |
| Native UI (egui/Slint) | Excellent performance, but rebuilding a 34-tab, chart-heavy, accessibility-audited workspace is slower than web UI work |

## Consequences

**Positive**

- Crawl engine, store and rules are compiled once and used by desktop, CLI, MCP and server.
- Installers in the tens of MB; no bundled browser runtime.
- Direct access to SQLite, crawl files, proxies and sidecar processes.
- The UI team works in React/Next with a normal component ecosystem.

**Negative / mitigations**

- **WebView differences** (WebView2, WKWebView, WebKitGTK): keep CSS conservative, test all three, add a WebKitGTK CI lane.
- **Static export limits**: no server components with runtime data; all data flows through IPC. Accepted — the crawl engine is local anyway.
- **IPC payload size**: page every result set; events carry progress only, never rows.
- **Linux WebKitGTK fragility**: document dependencies and pin a known-good distro in CI.

## Follow-ups

- ADR-0002: IPC paging contract (offset vs cursor) and event model.
- ADR-0003: rendering sidecar (bundled Chromium vs system browser discovery).
- Revisit only if a first-class web/server deployment becomes the primary product rather than an option.
