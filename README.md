# Coqui

**A modern, open-source reimplementation of Screaming Frog SEO Spider — Rust engine, Next.js workspace.**

Named after the *coquí*, the tiny frog native to Puerto Rico whose call is everywhere on the island and nowhere else.

![Coqui desktop shell: audit issues, robots-aware crawl results and the results grid](docs/images/coqui-m2.png)

> Coqui is an independent clean-room project. It is not affiliated with, endorsed by, or derived from Screaming Frog code or assets. "Screaming Frog" is a trademark of its owner.

## Status

**M2 pause & resume.** The parity target has been reverse-engineered and inventoried; the Tauri 2 shell, Rust crawl engine, robots.txt and redirect handling, six audit rules, a persisted frontier with pause/resume and the live Next.js results view are in place.

- [Feature inventory](docs/feature-inventory.md) — what the reference product does, measured
- [Development plan](docs/development-plan.md) — architecture, phases, estimates, risks
- [Reverse-engineering notes](docs/reverse-engineering.md) — evidence and limitations
- [Dev setup](docs/dev-setup.md) — run the desktop shell, tests and builds
- [Parity matrix](docs/reference/parity-matrix.csv) — every feature, phase, size, priority
- [Backlog](https://github.com/adamgreenwell/coqui/issues) and [project board](https://github.com/users/adamgreenwell/projects/16/views/1?layout_template=board)

### Backlog at a glance

| | |
| --- | --- |
| Issues | 85 (12 epics + 73 features) |
| Milestones | Phase 0 Foundations · Phase 1 Core parity · Phase 2 Advanced audit & UX · Phase 3 Integrations & AI · Phase 4 Hardening & 1.0 |
| Project fields | Epic · Phase · Priority · Size · Area · Status |
| Phase split | 16 / 20 / 22 / 11 / 4 features |
| Dependencies | Tracked per issue and listed in the [parity matrix](docs/reference/parity-matrix.csv) |
## What Coqui will be

A browser-based SEO crawler and audit workspace that matches the reference tool feature for feature:

- **Crawl** — HTTP/2 crawler with robots handling, URL normalisation, auth, scoping, pacing, pause/resume and portable crawl files.
- **Render** — headless Chromium over CDP: rendered DOM, screenshots, JS/CSS coverage, console diagnostics.
- **Audit** — SEO extraction, content and duplicate analysis, structured data, security, accessibility (axe), Lighthouse/PageSpeed, custom extraction/search/JS.
- **Analyse** — 34-tab results workspace, filtering, detail panes, dashboard, segments, graphs and word clouds.
- **Deliver** — 97 report types, bulk exports (CSV/XLSX/Sheets), XML/image sitemaps, email notifications.
- **Automate** — CLI, scheduler, MCP server for AI clients, headless container operation.
- **Enrich** — Search Console, GA4, PageSpeed, Ahrefs, Majestic, Moz, and any OpenAI/Anthropic/Gemini/Ollama model.

## Architecture

Tauri is the application shell. The Rust workspace runs in-process and owns crawling, storage and analysis; the Next.js UI is statically exported and rendered in the system WebView (WebView2 / WKWebView / WebKitGTK) from one codebase.

```
┌──────────────────────────── Tauri 2 shell ────────────────────────────┐
│  Next.js static export (apps/web)  ◀── IPC commands / events ──▶ Rust │
│                                                                       │
│  coqui-core   coqui-store   coqui-crawl   (later: render, audit, ...)  │
│  domain       SQLite        HTTP/frontier                             │
└───────────────────────────────────────────────────────────────────────┘
                    optional: coqui-cli · coqui-api (axum) · coqui-mcp
```

- **Shell:** Tauri 2 (Rust), WebView2/WKWebView/WebKitGTK, signed installers for Windows/macOS/Linux.
- **Backend:** Rust 2024, tokio, reqwest, scraper, rusqlite (SQLite), later Chromium/CDP and Lighthouse sidecars.
- **Frontend:** Next.js 16 App Router with `output: 'export'`, React 19, TanStack Query/Table/Virtual, Radix + Tailwind, d3/three.js, Monaco.
- **Shared core:** the same crates back the desktop app, the CLI and a future HTTP API, so parity logic lives once.
- **Data:** one portable SQLite crawl file per crawl; Postgres is a server-mode option, not a requirement.

## Repository layout

```
apps/web              Next.js workspace (planned)
crates/               Rust workspace (planned)
docs/                 inventory, plan, reverse-engineering notes, reference data
tools/plan/           backlog data and generators (single source of truth)
```

## Reproducing the inventory

The inventory is generated from evidence, not hand-maintained:

```bash
node tools/plan/build.mjs
```

This regenerates `docs/feature-inventory.md`, `docs/reference/*.csv` and the issue payloads from `tools/plan/*.mjs`.

## Contributing

The backlog is the roadmap. Pick an issue from the board, comment to claim it, and open a PR against the phase milestone. Every feature issue lists its acceptance criteria and the parity evidence behind it.

## License

To be decided by the project owner before the first code release (MIT or Apache-2.0 are the candidates). Until then, no license is granted for reuse.






