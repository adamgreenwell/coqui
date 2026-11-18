// Coqui parity backlog - AI, automation, configuration and quality.

export const featuresC = [
  // ---------------- E09 AI ----------------
  {
    id: 'N-01',
    epic: 'E09',
    title: 'AI provider abstraction',
    area: 'ai',
    phase: 3,
    size: 'L',
    priority: 'P2',
    summary:
      'A single interface over OpenAI, Anthropic, Gemini and Ollama with model catalogues, parameters and failure handling.',
    scope: [
      'Provider accounts: API key/base URL, model discovery, capability flags (vision, JSON mode, embeddings)',
      'Model parameters (temperature, top_p, max tokens, seed) with per-provider constraints',
      'Retries, timeouts, rate-limit backoff and provider fallback',
      'Usage accounting: tokens in/out, estimated cost, per-crawl totals',
      'Local Ollama support without keys',
    ],
    acceptance: [
      'Switching provider changes only configuration, not crawl semantics',
      'Invalid keys/model names produce actionable errors before a crawl starts',
      'Usage and cost are reported per crawl and per prompt',
      'A provider outage degrades to "AI unavailable" without failing the crawl',
    ],
    evidence:
      'OpenAI/Anthropic/Gemini/Ollama client classes, provider configs and model categories, advanced config files, AI usage strings.',
    deps: ['P-03'],
  },
  {
    id: 'N-02',
    epic: 'E09',
    title: 'Page prompts and prompt targets',
    area: 'ai',
    phase: 3,
    size: 'L',
    priority: 'P2',
    summary:
      'Ask questions of every page and store structured answers as crawl data, with controllable inputs.',
    scope: [
      'Prompt definitions with system/user templates and typed output (text, JSON, enum, number)',
      'Prompt targets: raw HTML, rendered text, extracted text, PDF content, URL details, custom extraction/JS values',
      'Per-page execution with batching, concurrency and cost caps',
      'Prompt tester with sample input and diffing',
      'Results as columns, tabs, issues and bulk exports',
    ],
    acceptance: [
      'A prompt can be tested on one URL before running on the crawl',
      'Structured outputs validate against the declared schema and quarantine invalid responses',
      'Token/cost caps stop execution before overspend',
      'Prompt results survive crawl save/reopen',
    ],
    evidence:
      'AI config (118 strings), prompt targets and content types, AI prompt tester, prompt dialogs, AI tab and bulk exports.',
    deps: ['N-01', 'X-07'],
  },
  {
    id: 'N-03',
    epic: 'E09',
    title: 'AI presets library',
    area: 'ai',
    phase: 3,
    size: 'M',
    priority: 'P2',
    summary:
      'Ready-made prompts for the tasks SEOs actually run: alt text, intent, language, sentiment, moderation and clustering.',
    scope: [
      'Presets per provider (alt text for images, intent of page, language of page, sentiment, moderation, embeddings)',
      'Preset parameters, output schema and example outputs',
      'Custom preset creation, versioning and sharing via config',
      'Presets surfaced in the UI with cost/latency expectations',
      'Evaluation fixtures for preset quality',
    ],
    acceptance: [
      'Every bundled preset runs unmodified on its target content type',
      'Presets are versioned so results can be traced to a prompt version',
      'Custom presets can be exported/imported with config',
      'Preset outputs are validated before storage',
    ],
    evidence:
      'ai_presets bundles for anthropic, gemini, ollama and openai (alt text, intent, language, sentiment, moderation, embeddings, clustering).',
    deps: ['N-02'],
  },
  {
    id: 'N-04',
    epic: 'E09',
    title: 'Embeddings, semantic similarity and clustering',
    area: 'ai',
    phase: 3,
    size: 'L',
    priority: 'P2',
    summary:
      'Vectorise page content, measure similarity and group pages into content clusters with an interactive map.',
    scope: [
      'Chunking strategies and embedding generation with batching and caching',
      'Vector storage (local index) and cosine similarity thresholds',
      'Low-relevance and semantically-similar-pages reports',
      'Cluster generation (k-means or graph) with configurable parameters',
      'Cluster map visualisation with labels and drill-down',
    ],
    acceptance: [
      'Embedding a 10k-page crawl completes with progress and resumability',
      'Similarity results are deterministic for the same inputs and model',
      'Clusters can be named, filtered and exported',
      'The cluster map renders 10k points with interaction above 30fps',
    ],
    evidence:
      'Embeddings config (29 strings), cosine/low-relevance settings, embedding visualisation bundle, semantic similarity exports.',
    deps: ['N-01', 'X-02'],
  },
  {
    id: 'N-05',
    epic: 'E09',
    title: 'AI governance and safety',
    area: 'ai',
    phase: 4,
    size: 'S',
    priority: 'P2',
    summary:
      'Budget, redact, audit and constrain AI usage so it is safe to enable on client crawls.',
    scope: [
      'Per-crawl and per-user spend budgets with hard stops',
      'PII/secret redaction before sending content to providers',
      'Provider data-retention notes and opt-in per integration',
      'Audit log of prompts, models and costs without storing sensitive content',
      'Kill switch and per-provider disable',
    ],
    acceptance: [
      'A crawl cannot exceed its configured AI budget',
      'Redaction rules remove configured patterns before transmission',
      'Disabling AI removes AI settings from new crawls without breaking old data',
      'Audit log answers "what was sent, to which model, at what cost" without content',
    ],
    evidence:
      'AI advanced config (rate limits, retries), usage strings, provider accounts, moderation preset.',
    deps: ['N-01', 'N-02'],
  },

  // ---------------- E10 Automation ----------------
  {
    id: 'T-01',
    epic: 'E10',
    title: 'Command-line interface',
    area: 'automation',
    phase: 0,
    size: 'L',
    priority: 'P0',
    summary:
      'Full headless parity: crawl, load, resume, compare, export, report, sitemap, email, project administration and logs.',
    scope: [
      'Crawl seeds: URL, list, sitemap, sitemap list, Google Sheet',
      'Load/resume/compare by crawl path or database id; pause/complete post-crawl actions',
      'Config and auth-config files; API enablement flags',
      'Export tabs, bulk exports, reports, sitemaps, custom summaries, email and attachments',
      'Output folder, filename patterns, overwrite/timestamped/append, skip-empty, save-crawl, save-logs, arg files',
    ],
    acceptance: [
      'Every documented option is implemented with the same argument grammar',
      'Exit codes distinguish success, partial success, config error and crawl failure',
      'CLI and UI produce identical export files for the same crawl',
      'A full crawl+export runs in a container with no GUI libraries',
    ],
    evidence:
      'CLI help capture (60+ options), command usage strings, scheduler headless requirements, arg-file support.',
    deps: ['C-01', 'B-02'],
  },
  {
    id: 'T-02',
    epic: 'E10',
    title: 'Scheduler and recurring tasks',
    area: 'automation',
    phase: 3,
    size: 'L',
    priority: 'P2',
    summary:
      'Schedule crawls with their configuration and post-actions, then track history and delivery.',
    scope: [
      'Task definition: seed, list source, config, auth config, schedule (interval/cron), post-actions',
      'Run now, enable/disable, duplicate and delete tasks',
      'Execution history with status, duration, output location and errors',
      'Notifications and Google Sheets/Data Studio delivery',
      'Missed-run handling and automatic repair of invalid tasks',
    ],
    acceptance: [
      'A scheduled crawl runs with the task configuration, not current defaults',
      'Overlapping runs are prevented or queued per policy',
      'History answers what ran, when, with which config and outputs',
      'Invalid tasks are flagged with a repair action before the next run',
    ],
    evidence:
      'Scheduling strings (135), start options, history, notifications, invalid-task repair, interval settings, CLI task-name.',
    deps: ['T-01', 'B-05'],
  },
  {
    id: 'T-03',
    epic: 'E10',
    title: 'MCP server for AI clients',
    area: 'automation',
    phase: 3,
    size: 'M',
    priority: 'P2',
    summary:
      'Expose crawl control and data to MCP clients over SSE/streamable HTTP, with a documented tool catalogue.',
    scope: [
      'Server lifecycle: start/stop, port, auto-start, reachability status',
      'Tool catalogue for crawl control, crawl discovery, tab queries, URL details, exports and reports',
      'Allowed-directory sandbox for file access; no filesystem access outside it',
      'Opt-in embedded Node runtime for script-based tools, with an explicit risk acknowledgement',
      'Downloadable tool documentation (Markdown) and in-app tool viewer',
    ],
    acceptance: [
      'An MCP client can start a crawl, poll status and fetch results without the UI',
      'Every advertised tool has a schema, description and example',
      'File access is confined to the configured directory',
      'Node-based tools stay disabled until explicitly enabled',
    ],
    evidence:
      'Inbuilt MCP server (SSE transport), mcp-core Spring Boot server, MCP configuration (port/directory/tool list), Node risk acknowledgement.',
    deps: ['T-01', 'P-01'],
  },
  {
    id: 'T-04',
    epic: 'E10',
    title: 'Headless API mode and CI operation',
    area: 'automation',
    phase: 4,
    size: 'M',
    priority: 'P2',
    summary:
      'Run Coqui as a service in CI: machine-readable output, stable exit codes, container images and health checks.',
    scope: [
      'Local HTTP API for crawl submission, status streaming and result queries',
      'JSON/NDJSON outputs and a machine-readable summary artifact',
      'Health/readiness endpoints and graceful shutdown',
      'Docker image with Chromium, Node and fonts; ARM64 support',
      'CI recipes for GitHub Actions and GitLab',
    ],
    acceptance: [
      'A pipeline can crawl a site, fail on a configured issue threshold and publish an artifact',
      'The container runs on linux/amd64 and linux/arm64',
      'Health checks distinguish starting, crawling and unhealthy states',
      'Logs are structured (JSON) with correlation ids',
    ],
    evidence:
      'API access mode, Spring Boot embedded server, CLI headless flags, child-process management and logs.',
    deps: ['T-01'],
  },

  // ---------------- E11 Configuration ----------------
  {
    id: 'G-01',
    epic: 'E11',
    title: 'Spider configuration experience',
    area: 'config',
    phase: 0,
    size: 'L',
    priority: 'P0',
    summary:
      'The crawl settings users touch most: crawl, extraction, rendering, limits, advanced, preferences and content.',
    scope: [
      'Crawl: depth, max URLs, subdomains, external links, robots, sitemaps, redirects, canonical handling',
      'Extraction: which element families to extract and store',
      'Rendering: enable, mode, waits, viewport, timeouts',
      'Limits: size, time, per-host, memory guards',
      'Advanced/preferences/content: hidden content, content area, anchors, parameters',
    ],
    acceptance: [
      'Every setting has a label, help text, type, default and validation',
      'Changes show a diff summary before a crawl starts',
      'Search across settings finds an option by name or description',
      'Settings are grouped and ordered to match analyst mental models',
    ],
    evidence:
      '321 spider configuration strings across crawl (66), extraction (76), rendering (55), advanced (39), preferences (30), content (28), limits (22).',
    deps: ['P-03'],
  },
  {
    id: 'G-02',
    epic: 'E11',
    title: 'Custom feature configuration',
    area: 'config',
    phase: 2,
    size: 'M',
    priority: 'P2',
    summary:
      'Authoring surfaces for custom extraction, custom search, custom JavaScript, link positions and URL path limits.',
    scope: [
      'List editors with add/remove/duplicate/reorder and inline validation',
      'Per-item editors with testers and sample previews',
      'Unique-name validation and import/export',
      'Empty/error states with guidance',
      'Bulk paste/import of definitions',
    ],
    acceptance: [
      'Invalid definitions cannot be saved and explain why',
      'Duplicate names are rejected with the conflicting item highlighted',
      'Every custom feature has a tester that runs without a crawl',
      'Definitions round-trip through config export/import',
    ],
    evidence:
      '130 custom configuration strings, custom validators (unique extraction/search/JS names, URL path limits), testers.',
    deps: ['G-01', 'A-06'],
  },
  {
    id: 'G-03',
    epic: 'E11',
    title: 'API and AI configuration experience',
    area: 'config',
    phase: 3,
    size: 'M',
    priority: 'P2',
    summary:
      'Connect accounts and tune providers without editing files, including validation and quota visibility.',
    scope: [
      'Account connection flows (OAuth, API keys) with status and expiry',
      'Metric/dimension selection per integration',
      'Date ranges, segments, device and country settings',
      'Model/provider configuration with capability display',
      'Connection tests and quota/usage indicators',
    ],
    acceptance: [
      'A user can connect every supported account from the UI without documentation',
      'Broken credentials are detected and explained before a crawl',
      'Configuration changes apply to the next crawl without a restart',
      'Secrets are write-only in the UI',
    ],
    evidence:
      '80 API configuration strings, account/metric/dimension/date-range windows, AI provider configuration and testers.',
    deps: ['G-01', 'I-05', 'N-01'],
  },
  {
    id: 'G-04',
    epic: 'E11',
    title: 'System settings, validation and testers',
    area: 'config',
    phase: 1,
    size: 'M',
    priority: 'P1',
    summary:
      'System-level controls and the safety rails that stop misconfiguration: storage, network, headers, auth, rewriting, includes/excludes and testers.',
    scope: [
      'Storage mode and location, crawl retention, memory limits',
      'Network/proxy, IPv4-only, embedded browser enable',
      'User agents (HTTP and robots), custom headers, Accept-Language',
      'Authentication, robots overrides, includes/excludes, URL rewriting, Markdown, CDN lists',
      'Validation testers with live match previews and error messages',
    ],
    acceptance: [
      'Storage changes are blocked while a crawl is active with an explanation',
      'Every regex-based setting has a tester with sample input and match highlighting',
      'Invalid configuration prevents crawl start with a field-level error',
      'Settings that require restart say so explicitly',
    ],
    evidence:
      'System, network, proxy, UA, headers, auth, robots, includes/excludes, rewriting, markdown and CDN configuration strings; regex testers.',
    deps: ['G-01'],
  },

  // ---------------- E12 Quality ----------------
  {
    id: 'Q-01',
    epic: 'E12',
    title: 'Parity harness and golden fixtures',
    area: 'quality',
    phase: 0,
    size: 'L',
    priority: 'P0',
    summary:
      'Measure parity continuously by comparing Coqui outputs with recorded reference outputs on a fixture corpus.',
    scope: [
      'Fixture sites covering statuses, redirects, canonicals, hreflang, JS rendering, structured data, images, PDFs, auth and accessibility',
      'Recorded reference exports committed as golden files (column-level CSV)',
      'Differential runner: crawl with Coqui, normalise, compare with tolerance rules',
      'Parity score per feature area, with allow-listed known differences',
      'CI gate that fails on unexplained regressions',
    ],
    acceptance: [
      'The harness runs locally and in CI with one command',
      'Every epic has at least one golden scenario',
      'Differences are classified (expected, missing, extra, value mismatch) with samples',
      'A parity report is published per release',
    ],
    evidence:
      'Deterministic CSV exports and column definitions from the reference tool provide the comparison surface.',
    deps: ['T-01', 'B-02'],
  },
  {
    id: 'Q-02',
    epic: 'E12',
    title: 'Performance and scale benchmarks',
    area: 'quality',
    phase: 4,
    size: 'M',
    priority: 'P2',
    summary:
      'Set and defend performance budgets for crawling, storage, querying and rendering.',
    scope: [
      'Benchmarks: URLs/second, memory per 100k URLs, DB size per URL, query latency, render throughput',
      'Synthetic 1M-URL crawl for storage/UI scale',
      'Regression tracking with thresholds in CI',
      'Profiling guides and flamegraphs',
      'Tuning documentation for operators',
    ],
    acceptance: [
      'Benchmarks run nightly and publish trends',
      'A 1M-URL crawl completes within the documented time and disk budget',
      'Query p95 for tab loads stays under 500ms at 1M rows',
      'Regressions over 10% fail the benchmark job',
    ],
    evidence:
      'Reference performance counters, HdrHistogram metrics, memory configuration, crawl-performance logging.',
    deps: ['P-01', 'U-01'],
  },
  {
    id: 'Q-03',
    epic: 'E12',
    title: 'Packaging and distribution',
    area: 'quality',
    phase: 0,
    size: 'M',
    priority: 'P1',
    summary:
      'Ship Coqui as something people can install and run: containers, desktop app and native CLI binaries.',
    scope: [
      'Docker/Compose deployment with persistent volumes',
      'Desktop build (Tauri) for Windows/macOS/Linux, or a documented local-server mode',
      'Prebuilt CLI binaries for the three platforms',
      'Auto-update strategy and versioning',
      'Signed artifacts and reproducible builds where possible',
    ],
    acceptance: [
      'A new user can run Coqui with one documented command',
      'The desktop build bundles Chromium/Node or downloads them on first run with checksums',
      'CLI binaries run without a JVM or Node install',
      'Releases are reproducible and signed',
    ],
    evidence:
      'Reference distribution model (installer, bundled JRE, Chromium and Node payloads) informs what must ship.',
    deps: [],
  },
  {
    id: 'Q-04',
    epic: 'E12',
    title: 'Documentation, migration and community',
    area: 'quality',
    phase: 4,
    size: 'M',
    priority: 'P2',
    summary:
      'Make Coqui learnable and make leaving the reference tool safe: guides, migration path, API docs and examples.',
    scope: [
      'User guide organised by task, not by menu',
      'Migration guide: config mapping, crawl import, feature parity table',
      'API/CLI/MCP reference generated from schemas',
      'Tutorials and sample projects',
      'Changelog and deprecation policy',
    ],
    acceptance: [
      'Every backlog feature links to user documentation when shipped',
      'The migration guide maps every reference setting to a Coqui setting',
      'API reference is generated and versioned with releases',
      'A new user can complete a first audit from the docs alone',
    ],
    evidence:
      'Reference user guide structure and feature surface, CLI help, configuration surface.',
    deps: ['Q-01'],
  },
];
