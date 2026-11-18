# Reverse-engineering notes

How the parity target was characterised, what was actually observed, and what remains unknown.

## Target

| Fact | Value |
| --- | --- |
| Product | Screaming Frog SEO Spider |
| Version | 25.1 |
| Build | `4a68a9139d6be0ce3eee7799e71945c5cb2c28e3` |
| Runtime | Eclipse Temurin OpenJDK 25.0.5 (bundled JRE) |
| UI toolkit | JavaFX 25.0.6-ea |
| Application artifact | `ScreamingFrogSEOSpider.jar` |
| SHA-256 | `3894c61083db5067dfd944d83edd70bc06f3a7a33abb7dd5a40b12cb6ffaa7ec` |
| Size / entries | 16,003,285 bytes; 6,430 zip entries; 6,105 class files |
| Install root | `C:\Program Files (x86)\Screaming Frog SEO Spider` |
| Bundled sidecars | Chromium 155.0.8059.40 (full + headless shell), Node.js 6.2 |

## Evidence collected

| # | Observation | Tool / source | Confidence |
| --- | --- | --- | --- |
| 1 | JAR is a ZIP archive with 6,430 entries | REA `open_binary` (archive route) | Observed |
| 2 | Container inventory: 6,428 nodes, 6,430 occurrences, 6,429 edges, zero integrity contradictions, source complete | REA `inspect_artifact` (summary) | Observed |
| 3 | Manifest: `Main-Class: uk.co.screamingfrog.seospider.SeoSpider`, Maven-built, `Class-Path` enumerating 280+ libraries | JAR manifest | Observed |
| 4 | Package structure: `uk/co/screamingfrog/seospider` (3,300), `uk/co/screamingfrog/utils` (1,048), `uk/co/screamingfrog/ui` (371), `uk/co/screamingfrog/db` (37), plus `seo/spider/*` packages | JAR entry listing | Observed |
| 5 | Localisation bundles: `SeoSpider_{en,de,es,fr,it,nl,tr}.properties` (≈0.55–0.63 MB each) and `Utils_*` | JAR resources | Observed |
| 6 | Complete menu/action vocabulary (239 keys), configuration vocabulary (1,144 keys / 182 namespaces), 220 column labels, 97 report entries, 42 bulk exports, 57 CLI options | Bundle + CLI parsing (`tools/plan/build.mjs`) | Derived (high) |
| 7 | Issue taxonomy: 9 families, 344 rule identifiers (axe 95, PageSpeed 19, standalone 180) | Bundle key analysis | Derived (high) |
| 8 | 34-tab workspace and 32 persisted column layouts (366 column placements) | Per-user state `tabPrefs_MainPanel_Spider`, `column_prefs/*.json` | Observed (runtime state) |
| 9 | Dashboard widget model (`CRAWL_AUDIT`, `ISSUE_LIST`, `SUMMARY_COUNT`, …) and grid layout schema v4 | `dashboard/default_layout.json` | Observed |
| 10 | Storage modes (memory/database), retention policy, project/crawl folders, lock files, crash reporting | `spider.config`, data directory, config strings | Observed |
| 11 | CLI contract: 57 options covering crawl/load/resume/compare/export/report/sitemap/email/projects/logs | `ScreamingFrogSEOSpiderCli.exe --help` | Observed |
| 12 | Startup sequence and subsystem names (Chromium, Node, scheduler, database, custom extraction, structured data, Lighthouse, MCP) | `trace.txt` debug groups and runtime log | Observed |
| 13 | Licensing gate: CLI exits with `Could not locate licence file` and performs no crawl | CLI run attempt (2026-11-18) | Observed |
| 14 | Rendering/audit dependency set: CDP client 4.0.0-sf1, Lighthouse scripts, axe-core 652 KB, AMP validator WASM 4 MB, Schema.org N-Triples + 30 Google TTL profiles, Jena/SHACL | JAR resources and `lib/` inventory | Observed |

### REA evidence identifiers

| Purpose | ID |
| --- | --- |
| Artifact binding | `3894c61083db5067dfd944d83edd70bc06f3a7a33abb7dd5a40b12cb6ffaa7ec` |
| Inventory evidence | `ev_550132b579258ee866872353a632b62cdbd4b726cf9f52be09e4a7220ece2c11` |
| Summary view | `ev_87833ff820337ed199d397ed54a5707584c9333f5cf0e94d969d9867ec9db64c` |
| Nested inventory record | `ev_c56234100ee95d269e42d7ef4de8ae5cc456a9ff78550004019d24861364ce86` |
| Manifest / graph digests | `agm_9cd6a9aaf495e85c9d530c597bbcace427df859e2068aaa9c5320d4e46f78a34` / `2ebd8b977ed0fe1df2c65507016dc3db8048ef7877229dbb2d0cc0fd999a553c` |

REA's archive inventory establishes container identity and completeness. It does **not** establish runtime behaviour, and its own limitations note that ZIP inventory covers central-directory entries only.

## What the binary tells us about the architecture

The product is a JavaFX desktop client with three notable embedded runtimes:

1. **Chromium via CDP.** The app ships `sf-chrome-win-intel` (210 MB) and a headless shell (109 MB), plus a `chromedevtools-client`. Rendering is therefore browser-identical, not a DOM shim.
2. **Node.js.** `sf-node` 6.2 powers Lighthouse and the Lighthouse script templates; the MCP server can optionally expose Node to AI clients.
3. **Spring Boot + Tomcat + MCP.** `spring-boot-*`, `tomcat-embed-*`, `mcp-core` and `mcp-json-jackson3` are on the classpath; the app can start an embedded HTTP server for its inbuilt MCP server and API surface.

Storage uses SQLite (JDBC 3.53.4) and Derby, consistent with the two storage modes (memory and database) and the portable crawl-file model.

## Capability map from dependencies

| Dependency | Feature implication |
| --- | --- |
| `chromedevtools-*`, `sf-chrome-*`, `sf-headless-*` | JS rendering, screenshots, coverage, console/network capture |
| `sf-node-*`, `lighthouse/*.mjs` | Local Lighthouse audits, PageSpeed scores |
| `axe-core` | Accessibility rule engine (95 rules) |
| `structured_data/*.nt`, `jena-*`, `shacl` | Schema.org parsing and Google rich-result validation |
| `amp/validator_wasm.js` | AMP validation |
| `pdfbox`, `poi`, `Saxon`, `commons-csv` | PDF extraction, XLSX/XLS export, report templating |
| `languagetool-*`, `sf-hunspell`, `lucene-*`, `opennlp-*` | Spelling, grammar, readability, tokenisation (30+ languages) |
| `minhash`, `RoaringBitmap`, `fastutil` | Near-duplicate detection and large result sets |
| `sqlite-jdbc`, `derby`, `dbutils` | Crawl storage and project database |
| `google-api-services-*`, `msal4j`, `microsoft-graph` | Search Console, GA4, Drive/Sheets/Gmail, Microsoft Graph |
| `jsoup`, `ph-css`, `tika` | HTML/CSS parsing and file-type detection |
| `openai`, `anthropic`, `gemini`, `ollama` clients + presets | AI prompts, embeddings, moderation |
| `visualisations/*` (d3, 3d-force-graph, word cloud) | Crawl trees, force-directed graphs, word clouds |
| `javafx-*`, `nsmenufx`, `controlsfx` | Desktop UI (not reproduced; Coqui is web-based) |

## Method

1. Enumerated the archive and classified every entry by package and resource type.
2. Extracted the manifest and semantic (non-obfuscated) class names to recover configuration domains, enums, storage and integration structure.
3. Parsed the English localisation bundles into logical keys, then grouped them into menus, configuration namespaces, tabs, columns, issues, reports, exports, CLI and scheduler.
4. Read per-user runtime state to recover the real tab set, default column layouts and dashboard model.
5. Captured the CLI help as the authoritative headless contract.
6. Attempted a headless crawl to obtain reference output; blocked by the licence gate (recorded as an explicit gap).
7. Built the backlog directly from the grouped evidence, with one feature per coherent capability rather than one per string.

Reproduction scripts live in `tools/plan/`:

```
node tools/plan/build.mjs   # regenerates docs/reference/* and the inventory
```

The raw extracted bundles and JAR listings are intentionally **not** committed (see `.gitignore`); only derived counts, identifiers and our own descriptions are published.

## Limitations and unknowns

- **Obfuscation.** Most identifiers are renamed (`id123…`). Package names and preserved semantic classes give structure, but algorithms (readability constants, normalisation corner cases, duplicate thresholds) are not visible. These must be characterised with conformance fixtures.
- **No reference crawl output.** The CLI refuses to run without a licence file, and the GUI window was not reachable from this remote session for automated crawling. Golden files must be recorded on a licensed machine before parity testing.
- **No decompilation of logic.** The investigation establishes surface and dependencies, not implementation. Feature semantics come from configuration vocabulary, UI structure, standards and observed behaviour.
- **Crawl-file schema unknown.** The internal schema was not observed. Import compatibility is a research task, not an assumption.
- **MCP tool catalogue unknown.** The tool list is a runtime artifact; it must be enumerated on a licensed install.
- **UI wording.** Values were used to understand capability; Coqui will write its own interface copy.

## Legal and ethical boundary

Screaming Frog is proprietary software. This project:

- does not copy or redistribute its code, JARs, assets, translations or report templates;
- does not circumvent licensing or the licence gate (the failed CLI run is recorded as a limitation, not worked around);
- reimplements publicly observable behaviour and uses open standards (HTML, HTTP, robots.txt, sitemaps, Schema.org, axe-core, Lighthouse);
- uses its own naming, copy and visual design.

Feature names in this repository are descriptive of behaviour Coqui intends to match and are not claims of affiliation.
