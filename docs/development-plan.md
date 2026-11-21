# Coqui Development Plan

Coqui is a modern, open-source reimplementation of Screaming Frog SEO Spider: a Rust crawl and analysis engine running inside a Tauri 2 desktop shell, with a Next.js workspace rendered in the system WebView. This plan turns the [feature inventory](feature-inventory.md) into an executable roadmap.

> **Implementation note (2026-11-21).** This plan was written on 2026-11-18 against the measured reference product. The build then moved faster than the phase estimate and the crate layout diverged: `coqui-parse` became extraction inside `coqui-crawl`; `coqui-export` became `coqui-report`; `coqui-jobs` became scheduler code in `coqui-store`/`coqui-cli`; `coqui-ai` is not built. `coqui-render`, `coqui-lighthouse`, `coqui-api` and `coqui-cli` exist. The frontend libraries in §3 (TanStack, Radix, Tailwind, d3, Monaco) were not adopted; the workspace is React 19 + Next 16 with hand-written CSS. Treat this document as the original plan; [README](../README.md) and [status](status.md) describe what shipped.

- **Backlog:** 12 epics and 74 feature issues (the 73 planned here plus R-08, added during implementation), tracked on the [Coqui project board](https://github.com/users/adamgreenwell/projects/16/views/1?layout_template=board).
- **Target:** behavioural and data parity with Screaming Frog SEO Spider 25.1.
- **Stack:** Tauri 2 shell, Rust (edition 2024) engine, Next.js 16 / React 19 static-export frontend, SQLite (Postgres for server mode), Chromium + Node sidecars for rendering and Lighthouse.

---

## 1. Principles

1. **Measure parity, never assume it.** Every epic has golden fixtures and a differential comparison against recorded reference output.
2. **Local-first, server-capable.** The default deployment is a single binary plus a crawl folder. The same core runs as a multi-user server with Postgres.
3. **Stream everything.** Crawls produce rows continuously; the UI reads pages, not whole result sets. No component may require a full crawl in memory.
4. **One source of truth for data.** The Rust core owns extraction and issue semantics; the frontend renders, never recomputes SEO logic.
5. **Clean-room.** Implementation comes from observed behaviour and public standards. No Screaming Frog code, assets, translations or proprietary copy is copied into this repository.
6. **Degrade cleanly.** Rendering, AI and third-party APIs are optional; every dependency has a documented failure mode.

## 2. Scope

**In scope:** every capability in the inventory — crawling, rendering, extraction, audit rules, results UI, reports/exports/sitemaps, integrations, AI, CLI/scheduler/MCP, configuration, project/storage management.

**Out of scope (deliberate):**

- Licence enforcement, licence leasing, trial limits and update nagging.
- Proprietary telemetry and usage analytics.
- Pixel-identical UI cloning. Coqui is a modern interpretation, not a skin.
- Copying Screaming Frog data files verbatim where the format is undocumented; import support is best-effort behind a documented boundary.

## 3. Architecture

Tauri is the shell, not an afterthought. One Rust workspace owns crawling, storage, rendering and analysis; the Next.js UI is a static export talking to it over Tauri IPC. The same crates are reused by the CLI and a future HTTP server, so parity logic exists once.

```
┌────────────────────────────── Tauri 2 application ──────────────────────────────┐
│                                                                                 │
│  apps/web (Next.js static export)                                               │
│    React 19 · TanStack Query/Table/Virtual · Radix · d3                        │
│         │  invoke(commands)                    ▲  events (progress, finished)   │
│         ▼                                      │                                │
│  src-tauri (commands, state, lifecycle) ───────┘                                │
│         │                                                                       │
│  ┌──────┴───────┬──────────────┬──────────────┬──────────────┬───────────────┐  │
│  │ coqui-crawl  │ coqui-render │ coqui-parse  │ coqui-audit  │ coqui-store   │  │
│  │ frontier     │ Chromium/CDP │ HTML/CSS/    │ rules,       │ SQLite        │  │
│  │ HTTP, robots │ screenshots  │ JSON-LD      │ issues       │ portable file │  │
│  └──────┬───────┴──────────────┴──────────────┴──────────────┴───────────────┘  │
│         │                                                                       │
│  coqui-jobs (scheduler) · coqui-export (CSV/XLSX/PDF) · coqui-ai (providers)   │
└─────────────────────────────────────────────────────────────────────────────────┘
            optional, same crates: coqui-cli · coqui-api (axum) · coqui-mcp
```

### Why Tauri

| Benefit | Consequence |
| --- | --- |
| One Rust core, three surfaces | Desktop, CLI and server share crawl/extraction/issue semantics; no logic duplicated in a Node backend |
| Small, native shell | Installers in the tens of MB rather than a bundled browser; system WebView on each OS |
| Real filesystem and process access | Portable crawl files, sidecar Chromium/Lighthouse, local SQLite without a server |
| Cross-platform from one codebase | Windows (WebView2), macOS (WKWebView), Linux (WebKitGTK) |
| Typed IPC | Commands and events are compiled and checked, unlike ad-hoc HTTP contracts |

**Trade-offs we accept:** WebView rendering differs slightly per OS (the UI must be tested on all three); IPC replaces HTTP for local calls, so large payloads must be paged rather than sent whole; Next.js runs as a static export, so no server components with runtime data fetching (all data access is client-side through IPC); Linux WebKitGTK is the weakest target and gets its own test lane.

### Crate layout

| Crate | Responsibility |
| --- | --- |
| `coqui-core` | Domain types, IDs, config schema, issue taxonomy, column registry, URL identity |
| `coqui-store` | SQLite (and later Postgres) persistence, migrations, paging queries, aggregates |
| `coqui-crawl` | Frontier, scheduler, HTTP transport, robots, scope, auth, pacing |
| `coqui-render` | Chromium lifecycle, CDP client, render queue, screenshots, axe injection |
| `coqui-parse` | HTML/CSS/JSON-LD extraction, content analysis, link graph |
| `coqui-audit` | Rule engine, issue overrides, accessibility/PageSpeed mapping |
| `coqui-export` | CSV/XLSX/PDF writers, sitemap builder, report templates |
| `coqui-ai` | Provider clients, prompt targets, embeddings, usage accounting |
| `coqui-jobs` | Scheduler, task history, pause/resume, crash recovery |
| `src-tauri` | Tauri commands, app state, lifecycle, events, menus, updater |
| `coqui-cli`, `coqui-mcp`, `coqui-api` | Headless and agent surfaces built on the same crates |
| `apps/web` | Next.js static-export workspace |

### Frontend layout

- App Router with `output: 'export'`; shell chrome is static, every data surface is a client component.
- TanStack Table + TanStack Virtual for million-row grids; **server-side paging/sort/filter through IPC**, never full result sets in the WebView.
- Zustand for ephemeral UI state; TanStack Query for IPC-backed server state; URL/hash as the source of truth for filters and tabs.
- Radix primitives + Tailwind; visx/ECharts for charts; d3 + three.js for graphs; Monaco for custom JS/CSS; dnd-kit for dashboards.
- IPC payloads are paginated and typed; events carry progress, not rows.
## 4. Storage model

SQLite is the portable crawl format (one file per crawl, WAL enabled), with Postgres for server mode. Core tables:

| Table | Purpose |
| --- | --- |
| `crawl`, `crawl_config`, `crawl_stats` | Crawl identity, frozen configuration, counters |
| `url` | Canonical URL row: status, headers, timing, indexability, content metrics |
| `url_html` | Title/meta/headings/directives/structured-data summary |
| `link` | Edge list: source, target, anchor, rel, position, follow |
| `image`, `asset` | Media and asset rows with dimensions and sizes |
| `issue` | Rule hits with severity, evidence pointer and first/last seen |
| `extraction_value` | Custom extraction/search/JS/AI values (typed) |
| `page_blob` | Raw HTML, rendered DOM, screenshots (content-addressed, capped) |
| `segment`, `segment_member` | Segment definitions and membership |
| `integration_metric` | GSC/GA4/PSI/backlink/AI values with provenance |
| `comparison`, `comparison_delta` | Crawl diff results |
| `schedule`, `schedule_run` | Scheduler tasks and history |
| `job`, `job_event` | Crawl jobs, progress and resumable frontier state |

Design rules: every table carries `crawl_id`; aggregates are materialised views refreshed after analysis; blobs are chunked and never loaded by list queries; large exports stream from a server-side cursor.

## 5. Parity strategy

Parity is graded, so progress is honest:

| Level | Claim | Evidence |
| --- | --- | --- |
| **L0 — Discovered** | Same URL set for a fixture | URL set diff = 0 after normalisation |
| **L1 — Extracted** | Same values for a field family | Column-level CSV diff within tolerance |
| **L2 — Audited** | Same issues with same severity | Issue key diff = 0 on fixtures |
| **L3 — Presented** | Same reports/exports usable interchangeably | Report/CSV schema comparison |
| **L4 — Operational** | Same CLI/scheduler/MCP workflows | Scripted end-to-end runs |

The harness (Q-01) records reference exports on a licensed machine, commits golden files, crawls the same fixtures with Coqui, normalises both sides (URL ordering, timestamps, volatile headers) and publishes a per-epic parity score. CI fails on unexplained regressions.

## 6. Roadmap

Effort assumes two full-time engineers (one Rust, one full-stack) plus design/QA support part-time. A solo build should multiply durations by ~1.8.

### Phase 0 — Foundations (weeks 1–8)

**Goal:** a crawl you can start, watch, stop, reopen and export.

- Workspace, CI, release pipeline skeleton, ADRs; Tauri shell with one command (`ping`) and a static-export page.
- Store and crawl file format; migrations; project/crawl lifecycle.
- Config engine with JSON round-trip and profiles.
- Frontier, HTTP transport, robots, URL normalisation.
- Core HTML extraction; results grid and filters (read path).
- CLI skeleton (`--crawl`, `--export-tabs`, `--output-folder`).
- Parity harness v0 with two fixture sites.

**Exit criteria:** crawl a 5k-URL fixture, pause/resume, reopen, export Internal CSV; golden diff for URL set and 12 core columns.

### Phase 1 — Core parity (weeks 9–20)

**Goal:** the daily-driver crawl and audit loop.

- Scope, auth, seeds, pacing; rendered DOM (basic rendering).
- Content, links, images, security extraction.
- Detail panes, issues/crawl overview.
- Issue engine + overrides + analysis pipeline.
- Reports v1, tab/bulk exports, sitemap generation.
- Spider/system configuration UI; scheduler v1.
- Crawl comparison.

**Exit criteria:** parity L1–L2 on the fixture corpus for internal/external/security/response/title/meta/heading/link/image/content areas; CLI produces the same files as the UI.

### Phase 2 — Advanced audit & UX (weeks 21–34)

**Goal:** the capabilities that make the product trusted by specialists.

- Full render queue and wait strategies; screenshots; raw-vs-rendered diff.
- Lighthouse/PSI, JS/CSS coverage, console diagnostics.
- Accessibility (axe) and PageSpeed issue families.
- Structured data/SHACL, AMP, mobile.
- Custom extraction/search/JS with testers and element picker.
- Segments, dashboard, visualisations, word clouds, cluster map.
- Theming and locales; custom feature configuration UI.

**Exit criteria:** L2–L3 on rendering, accessibility, structured data, custom features; 10k-URL crawl with visualisations at interactive speed.

### Phase 3 — Integrations & AI (weeks 35–46)

**Goal:** enrichment and agentic workflows.

- OAuth/account framework and Google Workspace.
- GSC, GA4, PSI, Ahrefs, Majestic, Moz.
- AI providers, prompts/targets, presets, embeddings, clustering, governance.
- Scheduler v2 with notifications and Sheets/Data Studio.
- MCP server and tool catalogue; API mode.

**Exit criteria:** each integration verified against a real account in a staging environment; AI crawl with cost caps and full audit trail.

### Phase 4 — Hardening & 1.0 (weeks 47–56)

**Goal:** trustworthy release.

- 1M-URL benchmarks and tuning; memory/disk budgets.
- Docker, desktop (Tauri) and CLI packaging; signing.
- Documentation, migration guide, tutorials.
- Full parity report; known-differences register; 1.0 release.

**Exit criteria:** parity report published with no unexplained L0/L1/L2 gaps on fixtures; install on all three OSes; docs complete for every shipped feature.

## 7. Milestones

| Milestone | Week | Content |
| --- | --- | --- |
| M0 Walking skeleton | 4 | Crawl → store → grid → CSV |
| M1 Alpha | 10 | Config, pause/resume, core extraction, CLI |
| M2 Beta | 20 | Audit engine, reports, full crawl config |
| M3 Feature complete | 34 | Rendering, accessibility, custom features, visualisations |
| M4 Integrations | 46 | GSC/GA4/PSI/backlinks, AI, scheduler, MCP |
| M5 1.0 | 56 | Performance, packaging, docs, parity sign-off |

## 8. Estimation

| Area | Feature count | Effort (engineer-weeks, 2-person team) |
| --- | --- | --- |
| Platform & storage | 7 | 14 |
| Crawl engine | 8 | 16 |
| Extraction | 8 | 18 |
| Rendering & audit execution | 7 | 16 |
| Results UI | 7 | 16 |
| Issue engine | 9 | 14 |
| Reports & exports | 5 | 9 |
| Integrations | 5 | 11 |
| AI | 5 | 8 |
| Automation | 4 | 9 |
| Configuration | 4 | 8 |
| Quality & release | 4 | 10 |
| **Total** | **73** | **~149 engineer-weeks** |

At ~2 engineers this is ~18 months including integration and hardening; the phase plan above compresses to ~13 months by parallelising frontend and backend and deferring integrations.

## 9. Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Chromium/Node packaging and licensing | Rendering cannot ship | Use system Chrome/`chrome-headless-shell` discovery with checksum-pinned download; document container image; no bundled proprietary code |
| Behavioural edge cases hidden by obfuscation | Silent parity gaps | Fixture corpus + differential testing; characterise normalisation/robots/readability empirically before coding |
| Third-party API changes | Integrations break | Adapter layer, contract tests with recorded fixtures, graceful degradation |
| SQLite write contention at scale | Slow crawls | Batched writers, WAL, single-writer actor per crawl; Postgres path for servers |
| Frontend grid performance | Unusable at scale | Server-side paging/sort/filter from day one; virtualisation budgets in CI |
| WebView differences (WebView2/WKWebView/WebKitGTK) | UI bugs on one platform | Visual + interaction test lane per OS; avoid exotic CSS; keep WebKitGTK in CI |
| Legal exposure from cloning | Project risk | Clean-room process, no copied assets/copy, trademark disclaimer, independent UI design |
| Scope creep (AI, MCP, dashboards) | 1.0 slips | Phase gates; integrations strictly after core parity |

## 10. Quality plan

- **Unit:** extraction fixtures per field family; rule fixtures per issue; normalisation tables.
- **Integration:** crawl a local fixture server with known HTML; assert DB rows and exported CSVs.
- **Differential:** reference exports vs Coqui exports, normalised and diffed (Q-01).
- **Performance:** nightly 100k/1M runs with budgets (Q-02).
- **Accessibility:** axe + keyboard tests on the Coqui UI itself.
- **Security:** secret scanning, dependency audit, sandbox tests for custom JS.
- **Release:** signed artifacts, reproducible builds, changelog.

## 11. First 90 days (concrete)

1. Week 1–2: workspace, CI, ADR-001 (storage), ADR-002 (config), ADR-003 (rendering sidecar).
2. Week 3–4: `coqui-store` + `coqui-crawl` walking skeleton; crawl fixture, export Internal CSV; M0 demo.
3. Week 5–6: config schema + JSON round-trip; projects/crawls; pause/resume.
4. Week 7–8: results grid read path with virtualised table and filters; parity harness v0.
5. Week 9–10: robots, URL normalisation conformance suite, includes/excludes.
6. Week 11–12: auth, seeds, pacing; M1 alpha release notes.

## 12. Decision log (ADRs to write)

1. Storage: SQLite per crawl with Postgres server mode (vs DuckDB/embedded KV).
2. Crawl file as SQLite vs custom columnar format.
3. Rendering sidecar: browserless/CDP direct vs Playwright driver.
4. Lighthouse: local Node runner vs PSI-only first.
5. Frontend data access: REST + SSE vs GraphQL.
6. Job execution: in-process tokio vs external queue.
7. Config schema versioning and migration policy.
8. Column registry as code vs database-driven metadata.
11. IPC paging contract (cursor vs offset) and event granularity for live crawls.
9. MCP tool surface and permission model.
10. Tauri as the primary shell, with the same crates exposed by CLI and an optional axum server.

