// Coqui parity backlog - epics.
// Everything here is derived from observable behaviour of Screaming Frog SEO Spider 25.1
// (build 4a68a913) and is written as a clean-room specification. No Screaming Frog code,
// assets or proprietary copy is reproduced.

export const epics = [
  {
    id: 'E01',
    slug: 'platform',
    title: 'Platform shell, projects & storage',
    summary:
      'The application substrate: how crawls are created, stored, reopened, versioned and shared, and the shell that hosts every mode.',
    outcome:
      'A user can create projects and crawls, store them durably, reopen them after a restart, and move them between machines without data loss.',
    phase: 0,
    size: 'XL',
    priority: 'P0',
    evidence:
      'JAR contains a Derby/SQLite storage stack, project/crawl instance data, lock files, crawl-retention configuration and a project-level database mode.',
  },
  {
    id: 'E02',
    slug: 'crawl-engine',
    title: 'Crawl engine',
    summary:
      'The URL frontier, HTTP transport, robots handling, URL normalisation, scoping, authentication and crawl pacing.',
    outcome:
      'Coqui crawls a site the way the reference tool does: same URLs discovered, same status codes, same redirects, comparable throughput and politeness.',
    phase: 0,
    size: 'XL',
    priority: 'P0',
    evidence:
      'JAR: SpiderCrawlConfig, SpiderInternalURLConfig, SpiderURLReWritingConfig, robots modes, auth configs, scheduler package, Chromium+Node runtime.',
  },
  {
    id: 'E03',
    slug: 'rendering',
    title: 'Rendering & browser runtime',
    summary:
      'Headless Chromium orchestration over CDP, render wait strategies, screenshots, Lighthouse, coverage and accessibility execution.',
    outcome:
      'JavaScript-rendered sites produce the same rendered DOM, screenshots, Lighthouse scores and accessibility results as the reference tool.',
    phase: 1,
    size: 'XL',
    priority: 'P1',
    evidence:
      'Bundled Chromium 155 and headless shell, chromedevtools-client, sf-node 6.2, Lighthouse scripts, axe-core, AMP validator wasm.',
  },
  {
    id: 'E04',
    slug: 'extraction',
    title: 'SEO extraction & result data model',
    summary:
      'Everything measured per URL: HTML elements, content, links, images, structured data, security, documents, AMP and mobile.',
    outcome:
      'The result model covers every column the reference tool exposes, with matching semantics, edge cases and null handling.',
    phase: 0,
    size: 'XL',
    priority: 'P0',
    evidence:
      '219 table-column strings, 31 per-tab column preference files, Schema.org + SHACL bundles, jsoup, ph-css, PDFBox, Tika, minhash.',
  },
  {
    id: 'E05',
    slug: 'results-ui',
    title: 'Results explorer & analysis UI',
    summary:
      'The 34-tab results workspace, filtering, detail panes, dashboard, segments and graph visualisations.',
    outcome:
      'An analyst can open a crawl and answer the same questions with the same views, filters and drill-downs, in a browser instead of a desktop app.',
    phase: 0,
    size: 'XL',
    priority: 'P0',
    evidence:
      'Main-panel tab preferences list 34 tabs; 31 column layouts; dashboard layout JSON with widget types; five graph visualisations; 247 visualisation strings.',
  },
  {
    id: 'E06',
    slug: 'audit-engine',
    title: 'Issue & audit engine',
    summary:
      'The rule engine that turns crawled data into errors, warnings, opportunities, accessibility violations and PageSpeed findings.',
    outcome:
      'Every reference issue type exists with comparable severity, evidence links, explanations and suppression controls.',
    phase: 1,
    size: 'XL',
    priority: 'P1',
    evidence:
      'Issue strings span axe (95 rules), PageSpeed (38), links (26), internal (22), images (16), GSC (16), mobile (12), external (8) plus standalone checks; issue override config.',
  },
  {
    id: 'E07',
    slug: 'reports',
    title: 'Reports, exports & sitemaps',
    summary:
      'Report generation, tab and bulk exports, spreadsheet delivery, sitemap generation and email notifications.',
    outcome:
      'A crawl can be delivered in every format the reference tool supports, with matching columns, filters and file naming.',
    phase: 1,
    size: 'L',
    priority: 'P1',
    evidence:
      '97 report menu entries, ~40 bulk exports, CSV/XLS/XLSX/Google Sheets formats, XML + image sitemaps, notification email templates.',
  },
  {
    id: 'E08',
    slug: 'integrations',
    title: 'Third-party data integrations',
    summary:
      'Search Console, GA4, PageSpeed, backlink providers and Google Workspace accounts and data joins.',
    outcome:
      'Crawl rows can be enriched with the same third-party metrics, joined on the same URL-matching rules.',
    phase: 3,
    size: 'L',
    priority: 'P2',
    evidence:
      '44 API classes (GSC, GA4, PSI, Ahrefs, Majestic, Moz), Microsoft Graph and Google API clients, OAuth secrets and account config windows.',
  },
  {
    id: 'E09',
    slug: 'ai',
    title: 'AI & semantic analysis',
    summary:
      'Provider-agnostic LLM prompts, embeddings, clustering and content analysis.',
    outcome:
      'Users can run page-level prompts and semantic clustering across a crawl with any supported provider, at controlled cost.',
    phase: 3,
    size: 'L',
    priority: 'P2',
    evidence:
      'OpenAI/Anthropic/Gemini/Ollama configs and clients, 37 AI config classes, AI presets, embeddings + cosine similarity + cluster visualisation.',
  },
  {
    id: 'E10',
    slug: 'automation',
    title: 'Automation: CLI, scheduler, MCP & API',
    summary:
      'Headless operation and machine interfaces: command line, scheduled tasks, an MCP server for AI clients and a local API.',
    outcome:
      'Coqui runs unattended in CI and cron, and agents can drive and query crawls programmatically.',
    phase: 0,
    size: 'L',
    priority: 'P1',
    evidence:
      'CLI exposes 60+ options (crawl/load/resume/compare/export/report/sitemap/email/project admin/logs); scheduler config and history; inbuilt MCP server with SSE transport and tool list.',
  },
  {
    id: 'E11',
    slug: 'configuration',
    title: 'Configuration & settings experience',
    summary:
      'The full settings surface: every option, grouped the way users expect, with validation and testers.',
    outcome:
      'Every reference setting has a home in Coqui with an equivalent type, default and effect, importable and exportable as config.',
    phase: 1,
    size: 'L',
    priority: 'P1',
    evidence:
      '1,097 configuration strings across ~60 sections (spider 321, custom 130, AI 118, APIs 80); JSON config validators; compare-configurations feature.',
  },
  {
    id: 'E12',
    slug: 'quality',
    title: 'Parity verification & release engineering',
    summary:
      'How we prove parity, keep it, and ship: differential fixtures, performance budgets, packaging and documentation.',
    outcome:
      'Parity is measured, not asserted. Every release reports a parity score with no unexplained regressions.',
    phase: 4,
    size: 'L',
    priority: 'P1',
    evidence:
      'Reference CLI exports provide machine-comparable output; crawl files provide schema-level comparison; the tool exposes deterministic CSV columns.',
  },
];
