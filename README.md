# Coqui

**A modern, open-source reimplementation of Screaming Frog SEO Spider — Rust engine, Next.js workspace.**

Named after the *coquí*, the tiny frog native to Puerto Rico whose call is everywhere on the island and nowhere else.

> Coqui is an independent clean-room project. It is not affiliated with, endorsed by, or derived from Screaming Frog code or assets. "Screaming Frog" is a trademark of its owner.

## Status

**Planning.** The parity target has been reverse-engineered and inventoried; the backlog and development plan are published. Implementation starts with the walking skeleton (crawl → store → grid → export).

- [Feature inventory](docs/feature-inventory.md) — what the reference product does, measured
- [Development plan](docs/development-plan.md) — architecture, phases, estimates, risks
- [Reverse-engineering notes](docs/reverse-engineering.md) — evidence and limitations
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

```
Next.js workspace  ──REST/SSE──  coqui-api (axum)
                                     │
        ┌────────────┬───────────────┼───────────────┬────────────┐
    coqui-crawl  coqui-render    coqui-parse     coqui-audit  coqui-store
    frontier     Chromium/CDP    HTML/CSS/JSON-LD rules        SQLite/Postgres
    HTTP, robots screenshots     links, content  issues       portable crawls
        │            │
    coqui-jobs   coqui-export            coqui-ai
    scheduler    CSV/XLSX/PDF/sitemaps   providers/embeddings
```

- **Backend:** Rust 2024, tokio, axum, sqlx (SQLite/Postgres), reqwest, scraper/lol_html, chromiumoxide or direct CDP.
- **Frontend:** Next.js 15 App Router, React 19, TanStack Query/Table/Virtual, Radix + Tailwind, d3/three.js, Monaco.
- **Sidecars:** Chromium (system or pinned download), Node only for Lighthouse.

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

