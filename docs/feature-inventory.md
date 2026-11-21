# Coqui Feature Inventory

A complete, evidence-based inventory of Screaming Frog SEO Spider **25.1** (build `4a68a913`) as the parity target for Coqui, an independent Rust + Next.js reimplementation.

> Coqui is an independent clean-room project. It is not affiliated with, endorsed by, or derived from Screaming Frog code or assets. Feature names below describe behaviour that Coqui intends to match.

## 1. How this inventory was produced

| Evidence | What it established |
| --- | --- |
| `ScreamingFrogSEOSpider.jar` (SHA-256 `3894c610...aa7ec`, 6,430 entries, 6,105 classes) | Application layer is a JavaFX desktop client with an embedded Chromium and Node runtime |
| `lib/` (280+ jars) | Exact third-party capabilities: CDP, Lighthouse, axe-core, Schema.org/SHACL, PDF, Office, OAuth/Google APIs, SQLite/Derby, NLP/spelling, graph/geometry |
| Localisation bundles (`SeoSpider_en`, `Utils_en`) | Complete UI surface: menus, tabs, settings, issues, reports, CLI, scheduler (5,500+ logical keys) |
| Per-user state (34 tab registers, 31 column layouts, dashboard layout, spider config) | Real information architecture and persisted defaults |
| CLI `--help` (captured) | 60+ command options and the headless contract |
| Runtime trace and process/config layout | Startup sequence, storage modes, lock files, scheduler and API subsystems |

Full evidence detail, including REA evidence identifiers and the limits of the analysis, is in [reverse-engineering.md](reverse-engineering.md). Machine-readable extracts are in `docs/reference/`.

## 2. Surface snapshot

| Dimension | Count |
| --- | --- |
| Menu/action strings | 239 |
| Configuration strings | 1144 across 182 sections |
| Result tabs | 34 |
| Per-tab column layouts | 32 (366 column placements) |
| Distinct result column labels | 220 |
| Issue families / rules | 9 / 344 |
| Report menu entries | 97 |
| Bulk exports | 42 |
| CLI options | 57 |
| Shipped locales | 7 |
| Backlog epics / features | 12 / 73 |

## 3. Product modes

| Mode | Purpose |
| --- | --- |
| Spider | Crawl a site from a seed and discover linked URLs |
| List | Audit a supplied list of URLs without link discovery |
| SERP | Capture and analyse search engine results pages |
| Compare | Diff two crawls (previous vs current, or two files/ids) |
| APIs | Inspect and use the local API/MCP surface and connected providers |

## 4. Result tabs and data model

The reference UI exposes **34 tabs**. Column counts below come from the persisted per-tab column layouts, which is the best available measure of how much data each view carries.

| Tab | Columns |
| --- | --- |
| Internal | 70 |
| JavaScript | 29 |
| Social | 21 |
| Content | 19 |
| AMP HTML | 18 |
| Accessibility | 15 |
| Links | 14 |
| Canonicals | 12 |
| Hreflang | 12 |
| Pagination | 11 |
| Validation | 11 |
| Meta Description | 10 |
| Response Codes | 10 |
| Security | 10 |
| Structured Data | 10 |
| URL | 10 |
| Directives | 9 |
| Images | 8 |
| All AI Providers | 7 |
| Page Titles | 7 |
| External | 6 |
| H1 | 6 |
| H2 | 6 |
| Sitemaps | 6 |
| Analytics | 5 |
| GSC | 5 |
| Custom JavaScript | 4 |
| Custom Search | 4 |
| Custom Extraction | 3 |
| Link Metrics | 3 |
| Mobile | 3 |
| PageSpeed | 2 |

The `Internal` tab alone tracks 70 columns. Across tabs the model covers: identity and crawl metadata, status/response, indexability and directives, titles/meta/headings, content metrics, canonicalisation, hreflang, pagination, links and anchors, images and media, JavaScript and rendering, security headers and cookies, structured data, AMP/mobile, PageSpeed/Lighthouse, accessibility, custom extraction/search/JS, analytics and Search Console, link metrics and AI results.

## 5. Crawl engine surface

Crawl behaviour is configured through the Spider settings (321 strings across crawl, extraction, rendering, limits, advanced, preferences and content). The engine exposes:

- Seeds: single URL, URL list, sitemap, sitemap list, Google Sheet, SERP mode
- Discovery: BFS with depth and max-URL caps, subdomain control, external-link following, link-position filters
- Politeness: max threads, URLs/second, auto-throttle, 429 retries, pause on unreachable host
- Protocol: HTTP/1.1 and HTTP/2, TLS controls, IPv4-only mode, proxies, custom headers, Accept-Language
- Permission: robots.txt modes (standard/custom/ignore), meta robots, X-Robots-Tag, nofollow
- Identity: URL normalisation, percent-encoding modes, parameter handling, regex rewrite, path limits
- Session: basic/digest auth, forms-based login with a recorded browser sequence, cookie jar
- Lifecycle: start/pause/stop/resume, persisted frontier, crawl files, database mode, projects
- Comparison: crawl-to-crawl diff with issue deltas

## 6. Rendering and audit pipeline

The rendered layer is a real Chromium (155) plus a Node 6.2 runtime, driven over CDP:

- Render triggering and wait strategies (load, network idle, XHR, selector, delay), viewport and device emulation
- Raw vs rendered DOM storage and diffing; JS-injected title/canonical/link detection
- Screenshots, full-page capture and bulk export
- Lighthouse locally through the bundled Node runtime, or PageSpeed Insights API
- JavaScript and CSS coverage, console logs, blocked resources and network diagnostics
- axe-core accessibility execution with per-violation evidence
- AMP validation through the bundled validator WebAssembly module

## 7. Issue and audit taxonomy

Rules are grouped into **9 families** (344 rule identifiers, counting description/tip pairs once):

| Family | Rules |
| --- | --- |
| standalone | 180 |
| axe | 95 |
| psi | 19 |
| links | 13 |
| internal | 11 |
| images | 8 |
| gsc | 8 |
| mobile | 6 |
| external | 4 |

Issues carry a severity type (error, warning, opportunity), a priority (high, medium, low), an explanation, a remediation tip and per-URL evidence. Severity can be overridden and individual rules suppressed per project.

## 8. Reports, exports and sitemaps

The reference offers **97 report menu entries**, **42 bulk exports**, tab exports in CSV/XLS/XLSX/Google Sheets, XML and image sitemaps, and email delivery. The full catalogue is in [report-catalogue.csv](reference/report-catalogue.csv) and [bulk-exports.csv](reference/bulk-exports.csv).

Report families include: crawl overview and comparison, issues overview and issue diffs, redirects and redirect/canonical chains, canonicals, hreflang (all/missing/non-200/non-canonical return links), pagination, orphan pages, links and anchor text, duplicate content, spelling and grammar, language errors, structured data (parse errors, validation summary, Google rich-result features), PageSpeed and opportunities, accessibility, JavaScript console/coverage, security, cookies, HTTP headers and SERP summaries.

## 9. Integrations

| Integration | Data |
| --- | --- |
| Google Search Console | Performance metrics, URL inspection, sitemaps, referring pages |
| Google Analytics 4 | Traffic/engagement metrics by page path, landing page and query-string variants |
| PageSpeed Insights | Hosted Lighthouse metrics and opportunities (alternative to local Lighthouse) |
| Ahrefs | Authority/backlink/traffic metrics and batch analysis |
| Majestic | Trust flow / citation flow and backlink metrics |
| Moz | Domain/page authority and link metrics |
| Google Drive / Sheets / Gmail | Export delivery and email notifications |
| OpenAI, Anthropic, Gemini, Ollama | Page prompts, alt text, intent/language/sentiment, embeddings, moderation |

All integrations follow an account model with OAuth or API key storage, per-integration metric selection, URL matching rules and quota handling.

## 10. AI and semantic analysis

The AI subsystem is a first-class part of the product (118 configuration strings), not a bolt-on:

- Provider abstraction with model catalogues, advanced parameters, retries and usage accounting
- Prompt targets: raw HTML, rendered content, extracted text, PDF content, URL details, custom extraction and custom JavaScript values
- Bundled presets per provider: alt text, intent, language, sentiment, moderation, embeddings and clustering
- Embeddings index, cosine similarity, low-relevance detection and a content-cluster visualisation
- A prompt tester and per-crawl cost/usage reporting

## 11. Automation surface

The CLI exposes 57 options covering crawl, load, resume, compare, project/crawl administration, config and auth config, exports, reports, bulk exports, sitemaps, email, logging and an argument file. The scheduler adds recurring tasks with history and repair. An inbuilt MCP server exposes crawl tools to AI clients over SSE with a configurable port, allowed directory and an opt-in Node runtime.

The full option list is in [cli-options.csv](reference/cli-options.csv).

## 12. Configuration surface

The settings surface is the largest single part of the product. Sections below are derived from configuration key namespaces with their string counts:

| Section | Strings |
| --- | --- |
| Spider extraction | 76 |
| Spider crawl | 66 |
| Spider rendering | 55 |
| Custom extraction | 53 |
| Ai config | 40 |
| Spider advanced | 39 |
| Spider preferences | 30 |
| Spider content | 28 |
| Custom search | 25 |
| Apis ga4 | 23 |
| Spider limits | 22 |
| Custom javascript | 19 |
| Custom linkpositions | 19 |
| Apis user account | 18 |
| Apis date range | 18 |
| Robots custom | 16 |
| Embeddings cosine | 16 |
| System mcp | 15 |
| Ai tester | 14 |
| Openai advanced | 14 |
| Gsc dimension | 14 |
| Ai advanced | 13 |
|  | 12 |
| Ai html prompt target | 11 |
| Gsc url inspection | 11 |
| Gemini advanced | 10 |
| Segments tester | 9 |
| Custom url path limits | 9 |
| Embeddings low relevance | 9 |
| Content spelling and grammar | 8 |
| Segments | 8 |
| Url rewriting normalisation | 8 |
| Content | 7 |
| Ai name | 7 |
| Majestic metrics | 7 |
| System mcp server | 7 |
| System language | 6 |
| Trusted certs discovery | 6 |
| Readonly | 6 |
| Url rewriting regex replace | 6 |
| Ai model name | 6 |
| Ai prompt dialog | 6 |
| Openai model category | 6 |
| Googleservice general | 6 |
| Auth forms | 6 |

The complete list is in [configuration-surface.csv](reference/configuration-surface.csv).

## 13. UX shell

- 34-tab results workspace with per-tab tables, filters and detail panes
- Issues overview and crawl overview panels with response-time and site-structure graphs
- Configurable dashboard with summary, issue-list, crawl-audit and chart widgets
- Five graph visualisations (crawl tree, directory tree, force-directed 2D, force-directed 3D, directory force-directed) plus body/anchor word clouds and an embedding cluster map
- Segments with a tester, per-segment results, sitemaps and dashboards
- Light/dark themes, accent colour, focus mode, pinned and detached tabs, saved column layouts
- Seven locales

## 14. Non-functional parity requirements

These are inferred from product behaviour and must be treated as requirements, not nice-to-haves:

1. **Scale:** desktop-class crawls in the hundreds of thousands to millions of URLs, with a database storage mode.
2. **Determinism:** the same seeds and configuration produce the same URL set and comparable values, independent of concurrency.
3. **Interruptibility:** pause/resume and crash recovery without data loss.
4. **Portability:** a crawl is a portable file; a project is a folder that can move between machines.
5. **Isolation:** credentials and API keys never leak into crawl data, exports or logs.
6. **Offline safety:** AI and third-party integrations are opt-in and degrade cleanly.
7. **Accessibility:** the product that audits accessibility must itself meet WCAG 2.2 AA.
8. **Performance:** tab loads under 500 ms at one million rows; exports stream in bounded memory.

## 15. Backlog structure

12 epics and 73 feature issues map this inventory onto a build plan. Phases: 0 foundations (16), 1 core parity (20), 2 advanced audit and UX (22), 3 integrations and AI (11), 4 hardening and 1.0 (4).

| Epic | Features | Phase |
| --- | --- | --- |
| E01 Platform shell, projects & storage | 7 | 0 |
| E02 Crawl engine | 8 | 0 |
| E03 Rendering & browser runtime | 7 | 1 |
| E04 SEO extraction & result data model | 8 | 0 |
| E05 Results explorer & analysis UI | 7 | 0 |
| E06 Issue & audit engine | 9 | 1 |
| E07 Reports, exports & sitemaps | 5 | 1 |
| E08 Third-party data integrations | 5 | 3 |
| E09 AI & semantic analysis | 5 | 3 |
| E10 Automation: CLI, scheduler, MCP & API | 4 | 0 |
| E11 Configuration & settings experience | 4 | 1 |
| E12 Parity verification & release engineering | 4 | 4 |

The full matrix, with size, priority and dependencies, is in [parity-matrix.csv](reference/parity-matrix.csv). The live GitHub board is canonical and may contain issues added after this plan (for example R-08).

## 16. Known gaps and open questions

- The reference CLI refuses to crawl without a licence file. A licensed 25.1 export was later captured and unzipped for the first parity diff (see [validation/2026-11-21-first-parity-diff.md](validation/2026-11-21-first-parity-diff.md)); this inventory itself predates that export.
- Java identifiers in most packages are obfuscated (`id...`). Structure comes from package names, preserved semantic classes, resource bundles and runtime state, not from decompiled logic. Exact algorithms (e.g. readability formula constants, normalisation edge cases) remain to be characterised behaviourally.
- The exact MCP tool catalogue is only visible at runtime in the app; it must be enumerated on a licensed install.
- Internal database schema (crawl file format) could not be inspected without producing a crawl; this is the first task of the parity harness.
- Licensing/business model is intentionally out of scope: Coqui is open source and will not implement licence enforcement, update nagging or usage telemetry.
