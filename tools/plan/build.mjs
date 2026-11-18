import fs from 'node:fs';
import path from 'node:path';
import { epics } from './epics.mjs';
import { featuresA } from './features-a.mjs';
import { featuresB } from './features-b.mjs';
import { featuresC } from './features-c.mjs';

const ROOT = path.resolve(import.meta.dirname, '../..');
const RESEARCH = path.join(ROOT, 'research');
const DOCS = path.join(ROOT, 'docs');
const REF = path.join(DOCS, 'reference');
const GEN = path.join(ROOT, 'tools/plan/.generated');

fs.mkdirSync(REF, { recursive: true });
fs.mkdirSync(GEN, { recursive: true });

const features = [...featuresA, ...featuresB, ...featuresC];
const epicById = new Map(epics.map((e) => [e.id, e]));

// ---------- reference data parsing ----------

function readProperties(file) {
  if (!fs.existsSync(file)) return new Map();
  const raw = fs.readFileSync(file, 'latin1').split(/\r?\n/);
  const out = new Map();
  let key = null;
  let value = '';
  const flush = () => {
    if (key !== null) out.set(key, unescapeJava(value).trim());
    key = null;
    value = '';
  };
  for (const line of raw) {
    if (key !== null && /^\s/.test(line)) {
      value += line.trimStart();
      if (!line.endsWith('\\')) flush();
      continue;
    }
    if (key !== null) flush();
    const t = line.trim();
    if (!t || t.startsWith('#') || t.startsWith('!')) continue;
    const idx = line.indexOf('=');
    if (idx < 0) continue;
    key = line.slice(0, idx).trim();
    value = line.slice(idx + 1).trim();
    if (!line.endsWith('\\')) flush();
  }
  flush();
  return out;
}

function unescapeJava(s) {
  return s
    .replace(/\\u([0-9a-fA-F]{4})/g, (_, h) => String.fromCharCode(parseInt(h, 16)))
    .replace(/\\n/g, '\n')
    .replace(/\\t/g, '\t')
    .replace(/\\\\/g, '\\');
}

const seo = readProperties(path.join(RESEARCH, 'seospider_en.properties.utf8'));
const utils = readProperties(path.join(RESEARCH, 'utils_en.properties.utf8'));
const allProps = new Map([...utils, ...seo]);

function keysStarting(prefix) {
  return [...allProps.keys()].filter((k) => k.startsWith(prefix));
}

function prettify(key, prefix) {
  let s = key.slice(prefix.length).replace(/^\./, '');
  s = s.replace(/\.(label|tooltip|title|intro|description|tip|summary)$/, '');
  s = s.replace(/[._]/g, ' ');
  s = s.replace(/([a-z0-9])([A-Z])/g, '$1 $2');
  s = s.replace(/\s+/g, ' ').trim();
  return s.charAt(0).toUpperCase() + s.slice(1);
}

const prefDir = path.join(process.env.USERPROFILE || '', '.ScreamingFrogSEOSpider', 'column_prefs');
let tabColumns = [];
if (fs.existsSync(prefDir)) {
  for (const f of fs.readdirSync(prefDir).filter((f) => f.endsWith('.json'))) {
    try {
      const j = JSON.parse(fs.readFileSync(path.join(prefDir, f), 'utf8'));
      const tab = f.replace(/_list\.json$/, '').replace(/_/g, ' ').replace(/ All$/, '');
      tabColumns.push({ tab, columns: j.columns.length });
    } catch {}
  }
}
tabColumns.sort((a, b) => b.columns - a.columns);

const tabPrefsPath = path.join(process.env.USERPROFILE || '', '.ScreamingFrogSEOSpider', 'tabPrefs_MainPanel_Spider');
let tabs = [];
if (fs.existsSync(tabPrefsPath)) {
  try {
    const j = JSON.parse(fs.readFileSync(tabPrefsPath, 'utf8'));
    tabs = j.visible_tabs.map((t) => t.devName);
  } catch {}
}
if (!tabs.length) {
  tabs = ['Dashboard', 'Internal', 'External', 'Security', 'Response Codes', 'URL', 'Page Titles', 'Meta Description', 'Meta Keywords', 'H1', 'H2', 'Content', 'Images', 'Canonicals', 'Pagination', 'Directives', 'Hreflang', 'JavaScript', 'Links', 'AMP', 'Structured Data', 'Social', 'Sitemaps', 'PageSpeed', 'Mobile', 'Accessibility', 'Custom Search', 'Custom Extraction', 'Custom JavaScript', 'Analytics', 'Search Console', 'Validation', 'Link Metrics', 'AI'];
}

const configKeys = keysStarting('configuration.');
const configSections = new Map();
for (const k of configKeys) {
  const parts = k.split('.');
  const section = parts.slice(0, Math.min(3, parts.length - 1)).join('.');
  configSections.set(section, (configSections.get(section) || 0) + 1);
}
const configSectionRows = [...configSections.entries()]
  .map(([section, count]) => ({ section: prettify(section, 'configuration.'), count }))
  .sort((a, b) => b.count - a.count);

const issueRules = new Map();
const issueSuffixes = new Set(['description', 'tip', 'help', 'example', 'title', 'message']);
for (const k of keysStarting('issue.')) {
  const parts = k.split('.');
  const last = parts[parts.length - 1];
  if (!issueSuffixes.has(last)) continue;
  let family = 'standalone';
  let rule;
  if (parts.length >= 4) {
    family = parts[1];
    rule = parts.slice(1, -1).join('.');
  } else if (parts.length === 3) {
    rule = parts[1];
  } else {
    continue;
  }
  if (!issueRules.has(family)) issueRules.set(family, new Set());
  issueRules.get(family).add(rule);
}
const issueRows = [...issueRules.entries()]
  .map(([family, rules]) => ({ family, rules: rules.size }))
  .filter((r) => r.family === 'standalone' || r.rules >= 3)
  .sort((a, b) => b.rules - a.rules);

const reportKeys = [...keysStarting('menu.reports.'), ...keysStarting('menu.report.')];
const reportNames = reportKeys.map((k) => prettify(k, k.startsWith('menu.reports.') ? 'menu.reports.' : 'menu.report.'));
const bulkExportNames = keysStarting('menu.bulkexport.').map((k) => prettify(k, 'menu.bulkexport.'));
const menuCount = keysStarting('menu.').length;

let cliOptions = [];
const cliHelpPath = path.join(RESEARCH, 'cli-help.txt');
if (fs.existsSync(cliHelpPath)) {
  const help = fs.readFileSync(cliHelpPath, 'utf8');
  cliOptions = [...help.matchAll(/^\s{4}(--[a-z0-9-]+|-[a-z])/gm)].map((m) => m[1]);
}

const metrics = {
  reference: {
    product: 'Screaming Frog SEO Spider',
    version: '25.1',
    build: '4a68a9139d6be0ce3eee7799e71945c5cb2c28e3',
    jarSha256: '3894c61083db5067dfd944d83edd70bc06f3a7a33abb7dd5a40b12cb6ffaa7ec',
    jarEntries: 6430,
    classes: 6105,
  },
  surface: {
    menuStrings: menuCount,
    configurationStrings: configKeys.length,
    configurationSections: configSections.size,
    tabs: tabs.length,
    columnLayouts: tabColumns.length,
    columnsTrackedPerTab: tabColumns.reduce((n, t) => n + t.columns, 0),
    distinctColumnStrings: keysStarting('table.column.').length,
    issueFamilies: issueRows.length,
    issueRules: issueRows.reduce((n, r) => n + r.rules, 0),
    reports: reportNames.length,
    bulkExports: bulkExportNames.length,
    cliOptions: cliOptions.length,
    integrations: configKeys.filter((k) => k.startsWith('configuration.apis.')).length,
    aiStrings: configKeys.filter((k) => k.startsWith('configuration.ai.')).length,
    locales: 7,
  },
  backlog: {
    epics: epics.length,
    features: features.length,
    byPhase: Object.fromEntries([0, 1, 2, 3, 4].map((p) => [p, features.filter((f) => f.phase === p).length])),
    byArea: Object.fromEntries([...new Set(features.map((f) => f.area))].map((a) => [a, features.filter((f) => f.area === a).length])),
  },
};

fs.writeFileSync(path.join(REF, 'metrics.json'), JSON.stringify(metrics, null, 2));
fs.writeFileSync(
  path.join(REF, 'tabs-and-columns.csv'),
  'tab,columns\n' + tabColumns.map((t) => `${t.tab},${t.columns}`).join('\n') + '\n',
);
fs.writeFileSync(
  path.join(REF, 'configuration-surface.csv'),
  'section,strings\n' + configSectionRows.map((r) => `"${r.section}",${r.count}`).join('\n') + '\n',
);
fs.writeFileSync(
  path.join(REF, 'issue-families.csv'),
  'family,rules\n' + issueRows.map((r) => `${r.family},${r.rules}`).join('\n') + '\n',
);
fs.writeFileSync(path.join(REF, 'cli-options.csv'), 'option\n' + cliOptions.join('\n') + '\n');
fs.writeFileSync(path.join(REF, 'report-catalogue.csv'), 'report\n' + reportNames.map((r) => `"${r}"`).join('\n') + '\n');
fs.writeFileSync(path.join(REF, 'bulk-exports.csv'), 'export\n' + bulkExportNames.map((r) => `"${r}"`).join('\n') + '\n');

// ---------- parity matrix ----------

const matrixRows = features.map((f) => ({
  id: f.id,
  epic: epicById.get(f.epic).title,
  title: f.title,
  area: f.area,
  phase: f.phase,
  size: f.size,
  priority: f.priority,
  status: 'Not started',
  depends: f.deps.join(' '),
}));
const esc = (v) => `"${String(v).replace(/"/g, '""')}"`;
fs.writeFileSync(
  path.join(REF, 'parity-matrix.csv'),
  'id,epic,title,area,phase,size,priority,status,depends\n' +
    matrixRows.map((r) => [r.id, r.epic, r.title, r.area, r.phase, r.size, r.priority, r.status, r.depends].map(esc).join(',')).join('\n') +
    '\n',
);

// ---------- issues payload ----------

const phaseNames = {
  0: 'Phase 0 - Foundations',
  1: 'Phase 1 - Core parity',
  2: 'Phase 2 - Advanced audit and UX',
  3: 'Phase 3 - Integrations and AI',
  4: 'Phase 4 - Hardening and 1.0',
};

function featureBody(f) {
  const epic = epicById.get(f.epic);
  return [
    '## Summary',
    f.summary,
    '',
    `**Epic:** ${f.epic} - ${epic.title}`,
    `**Phase:** ${f.phase} - Size: ${f.size} - Priority: ${f.priority} - Area: ${f.area}`,
    '',
    '## Scope',
    ...f.scope.map((s) => `- ${s}`),
    '',
    '## Acceptance criteria',
    ...f.acceptance.map((s) => `- [ ] ${s}`),
    '',
    '## Parity evidence',
    f.evidence,
    '',
    '## Dependencies',
    f.deps.length ? f.deps.map((d) => `- ${d}`).join('\n') : '- None',
    '',
    `<!--EPIC:${f.epic}-->`,
    '',
    '_Generated from the Coqui parity inventory. Screaming Frog is a trademark of its owner; Coqui is an independent clean-room implementation._',
  ].join('\n');
}

function epicBody(e, childKeys) {
  return [
    '## Outcome',
    e.outcome,
    '',
    '## Summary',
    e.summary,
    '',
    `**Phase:** ${e.phase} - Size: ${e.size} - Priority: ${e.priority}`,
    '',
    '## Parity evidence',
    e.evidence,
    '',
    '## Features in this epic',
    ...childKeys.map((k) => `- [ ] ${k}`),
    '',
    '## Definition of done',
    '- All child features closed with acceptance criteria met',
    '- Covered by at least one golden parity scenario',
    '- User documentation published for every shipped capability',
  ].join('\n');
}

const epicIssues = epics.map((e) => ({
  key: e.id,
  type: 'epic',
  title: `EPIC ${e.id}: ${e.title}`,
  body: epicBody(e, features.filter((f) => f.epic === e.id).map((f) => `${f.id} - ${f.title}`)),
  labels: ['type/epic', `epic/${e.slug}`, `phase/${e.phase}`, `size/${e.size}`, `priority/${e.priority}`],
  milestone: phaseNames[e.phase],
  phase: e.phase,
  size: e.size,
  priority: e.priority,
  epic: e.id,
}));

const featureIssues = features.map((f) => ({
  key: f.id,
  type: 'feature',
  title: `${f.id} ${f.title}`,
  body: featureBody(f),
  labels: ['type/feature', `epic/${epicById.get(f.epic).slug}`, `area/${f.area}`, `phase/${f.phase}`, `size/${f.size}`, `priority/${f.priority}`],
  milestone: phaseNames[f.phase],
  phase: f.phase,
  size: f.size,
  priority: f.priority,
  epic: f.epic,
}));

fs.writeFileSync(path.join(GEN, 'issues.json'), JSON.stringify({ epics: epicIssues, features: featureIssues, phaseNames }, null, 2));

// ---------- feature inventory document ----------

const md = [];
const p = (s = '') => md.push(s);

p('# Coqui Feature Inventory');
p();
p('A complete, evidence-based inventory of Screaming Frog SEO Spider **25.1** (build `4a68a913`) as the parity target for Coqui, an independent Rust + Next.js reimplementation.');
p();
p('> Coqui is an independent clean-room project. It is not affiliated with, endorsed by, or derived from Screaming Frog code or assets. Feature names below describe behaviour that Coqui intends to match.');
p();
p('## 1. How this inventory was produced');
p();
p('| Evidence | What it established |');
p('| --- | --- |');
p('| `ScreamingFrogSEOSpider.jar` (SHA-256 `3894c610...aa7ec`, 6,430 entries, 6,105 classes) | Application layer is a JavaFX desktop client with an embedded Chromium and Node runtime |');
p('| `lib/` (280+ jars) | Exact third-party capabilities: CDP, Lighthouse, axe-core, Schema.org/SHACL, PDF, Office, OAuth/Google APIs, SQLite/Derby, NLP/spelling, graph/geometry |');
p('| Localisation bundles (`SeoSpider_en`, `Utils_en`) | Complete UI surface: menus, tabs, settings, issues, reports, CLI, scheduler (5,500+ logical keys) |');
p('| Per-user state (34 tab registers, 31 column layouts, dashboard layout, spider config) | Real information architecture and persisted defaults |');
p('| CLI `--help` (captured) | 60+ command options and the headless contract |');
p('| Runtime trace and process/config layout | Startup sequence, storage modes, lock files, scheduler and API subsystems |');
p();
p(`Full evidence detail, including REA evidence identifiers and the limits of the analysis, is in [reverse-engineering.md](reverse-engineering.md). Machine-readable extracts are in \`docs/reference/\`.`);
p();
p('## 2. Surface snapshot');
p();
p('| Dimension | Count |');
p('| --- | --- |');
p(`| Menu/action strings | ${metrics.surface.menuStrings} |`);
p(`| Configuration strings | ${metrics.surface.configurationStrings} across ${metrics.surface.configurationSections} sections |`);
p(`| Result tabs | ${metrics.surface.tabs} |`);
p(`| Per-tab column layouts | ${metrics.surface.columnLayouts} (${metrics.surface.columnsTrackedPerTab} column placements) |`);
p(`| Distinct result column labels | ${metrics.surface.distinctColumnStrings} |`);
p(`| Issue families / rules | ${metrics.surface.issueFamilies} / ${metrics.surface.issueRules} |`);
p(`| Report menu entries | ${metrics.surface.reports} |`);
p(`| Bulk exports | ${metrics.surface.bulkExports} |`);
p(`| CLI options | ${metrics.surface.cliOptions} |`);
p(`| Shipped locales | ${metrics.surface.locales} |`);
p(`| Backlog epics / features | ${metrics.backlog.epics} / ${metrics.backlog.features} |`);
p();
p('## 3. Product modes');
p();
p('| Mode | Purpose |');
p('| --- | --- |');
p('| Spider | Crawl a site from a seed and discover linked URLs |');
p('| List | Audit a supplied list of URLs without link discovery |');
p('| SERP | Capture and analyse search engine results pages |');
p('| Compare | Diff two crawls (previous vs current, or two files/ids) |');
p('| APIs | Inspect and use the local API/MCP surface and connected providers |');
p();
p('## 4. Result tabs and data model');
p();
p(`The reference UI exposes **${tabs.length} tabs**. Column counts below come from the persisted per-tab column layouts, which is the best available measure of how much data each view carries.`);
p();
p('| Tab | Columns |');
p('| --- | --- |');
for (const t of tabColumns) p(`| ${t.tab} | ${t.columns} |`);
p();
p('The `Internal` tab alone tracks 70 columns. Across tabs the model covers: identity and crawl metadata, status/response, indexability and directives, titles/meta/headings, content metrics, canonicalisation, hreflang, pagination, links and anchors, images and media, JavaScript and rendering, security headers and cookies, structured data, AMP/mobile, PageSpeed/Lighthouse, accessibility, custom extraction/search/JS, analytics and Search Console, link metrics and AI results.');
p();
p('## 5. Crawl engine surface');
p();
p('Crawl behaviour is configured through the Spider settings (321 strings across crawl, extraction, rendering, limits, advanced, preferences and content). The engine exposes:');
p();
p('- Seeds: single URL, URL list, sitemap, sitemap list, Google Sheet, SERP mode');
p('- Discovery: BFS with depth and max-URL caps, subdomain control, external-link following, link-position filters');
p('- Politeness: max threads, URLs/second, auto-throttle, 429 retries, pause on unreachable host');
p('- Protocol: HTTP/1.1 and HTTP/2, TLS controls, IPv4-only mode, proxies, custom headers, Accept-Language');
p('- Permission: robots.txt modes (standard/custom/ignore), meta robots, X-Robots-Tag, nofollow');
p('- Identity: URL normalisation, percent-encoding modes, parameter handling, regex rewrite, path limits');
p('- Session: basic/digest auth, forms-based login with a recorded browser sequence, cookie jar');
p('- Lifecycle: start/pause/stop/resume, persisted frontier, crawl files, database mode, projects');
p('- Comparison: crawl-to-crawl diff with issue deltas');
p();
p('## 6. Rendering and audit pipeline');
p();
p('The rendered layer is a real Chromium (155) plus a Node 6.2 runtime, driven over CDP:');
p();
p('- Render triggering and wait strategies (load, network idle, XHR, selector, delay), viewport and device emulation');
p('- Raw vs rendered DOM storage and diffing; JS-injected title/canonical/link detection');
p('- Screenshots, full-page capture and bulk export');
p('- Lighthouse locally through the bundled Node runtime, or PageSpeed Insights API');
p('- JavaScript and CSS coverage, console logs, blocked resources and network diagnostics');
p('- axe-core accessibility execution with per-violation evidence');
p('- AMP validation through the bundled validator WebAssembly module');
p();
p('## 7. Issue and audit taxonomy');
p();
p(`Rules are grouped into **${issueRows.length} families** (${metrics.surface.issueRules} rule identifiers, counting description/tip pairs once):`);
p();
p('| Family | Rules |');
p('| --- | --- |');
for (const r of issueRows) p(`| ${r.family} | ${r.rules} |`);
p();
p('Issues carry a severity type (error, warning, opportunity), a priority (high, medium, low), an explanation, a remediation tip and per-URL evidence. Severity can be overridden and individual rules suppressed per project.');
p();
p('## 8. Reports, exports and sitemaps');
p();
p(`The reference offers **${reportNames.length} report menu entries**, **${bulkExportNames.length} bulk exports**, tab exports in CSV/XLS/XLSX/Google Sheets, XML and image sitemaps, and email delivery. The full catalogue is in [report-catalogue.csv](reference/report-catalogue.csv) and [bulk-exports.csv](reference/bulk-exports.csv).`);
p();
p('Report families include: crawl overview and comparison, issues overview and issue diffs, redirects and redirect/canonical chains, canonicals, hreflang (all/missing/non-200/non-canonical return links), pagination, orphan pages, links and anchor text, duplicate content, spelling and grammar, language errors, structured data (parse errors, validation summary, Google rich-result features), PageSpeed and opportunities, accessibility, JavaScript console/coverage, security, cookies, HTTP headers and SERP summaries.');
p();
p('## 9. Integrations');
p();
p('| Integration | Data |');
p('| --- | --- |');
p('| Google Search Console | Performance metrics, URL inspection, sitemaps, referring pages |');
p('| Google Analytics 4 | Traffic/engagement metrics by page path, landing page and query-string variants |');
p('| PageSpeed Insights | Hosted Lighthouse metrics and opportunities (alternative to local Lighthouse) |');
p('| Ahrefs | Authority/backlink/traffic metrics and batch analysis |');
p('| Majestic | Trust flow / citation flow and backlink metrics |');
p('| Moz | Domain/page authority and link metrics |');
p('| Google Drive / Sheets / Gmail | Export delivery and email notifications |');
p('| OpenAI, Anthropic, Gemini, Ollama | Page prompts, alt text, intent/language/sentiment, embeddings, moderation |');
p();
p('All integrations follow an account model with OAuth or API key storage, per-integration metric selection, URL matching rules and quota handling.');
p();
p('## 10. AI and semantic analysis');
p();
p(`The AI subsystem is a first-class part of the product (${metrics.surface.aiStrings} configuration strings), not a bolt-on:`);
p();
p('- Provider abstraction with model catalogues, advanced parameters, retries and usage accounting');
p('- Prompt targets: raw HTML, rendered content, extracted text, PDF content, URL details, custom extraction and custom JavaScript values');
p('- Bundled presets per provider: alt text, intent, language, sentiment, moderation, embeddings and clustering');
p('- Embeddings index, cosine similarity, low-relevance detection and a content-cluster visualisation');
p('- A prompt tester and per-crawl cost/usage reporting');
p();
p('## 11. Automation surface');
p();
p(`The CLI exposes ${cliOptions.length} options covering crawl, load, resume, compare, project/crawl administration, config and auth config, exports, reports, bulk exports, sitemaps, email, logging and an argument file. The scheduler adds recurring tasks with history and repair. An inbuilt MCP server exposes crawl tools to AI clients over SSE with a configurable port, allowed directory and an opt-in Node runtime.`);
p();
p('The full option list is in [cli-options.csv](reference/cli-options.csv).');
p();
p('## 12. Configuration surface');
p();
p('The settings surface is the largest single part of the product. Sections below are derived from configuration key namespaces with their string counts:');
p();
p('| Section | Strings |');
p('| --- | --- |');
for (const s of configSectionRows.slice(0, 45)) p(`| ${s.section} | ${s.count} |`);
p();
p('The complete list is in [configuration-surface.csv](reference/configuration-surface.csv).');
p();
p('## 13. UX shell');
p();
p('- 34-tab results workspace with per-tab tables, filters and detail panes');
p('- Issues overview and crawl overview panels with response-time and site-structure graphs');
p('- Configurable dashboard with summary, issue-list, crawl-audit and chart widgets');
p('- Five graph visualisations (crawl tree, directory tree, force-directed 2D, force-directed 3D, directory force-directed) plus body/anchor word clouds and an embedding cluster map');
p('- Segments with a tester, per-segment results, sitemaps and dashboards');
p('- Light/dark themes, accent colour, focus mode, pinned and detached tabs, saved column layouts');
p('- Seven locales');
p();
p('## 14. Non-functional parity requirements');
p();
p('These are inferred from product behaviour and must be treated as requirements, not nice-to-haves:');
p();
p('1. **Scale:** desktop-class crawls in the hundreds of thousands to millions of URLs, with a database storage mode.');
p('2. **Determinism:** the same seeds and configuration produce the same URL set and comparable values, independent of concurrency.');
p('3. **Interruptibility:** pause/resume and crash recovery without data loss.');
p('4. **Portability:** a crawl is a portable file; a project is a folder that can move between machines.');
p('5. **Isolation:** credentials and API keys never leak into crawl data, exports or logs.');
p('6. **Offline safety:** AI and third-party integrations are opt-in and degrade cleanly.');
p('7. **Accessibility:** the product that audits accessibility must itself meet WCAG 2.2 AA.');
p('8. **Performance:** tab loads under 500 ms at one million rows; exports stream in bounded memory.');
p();
p('## 15. Backlog structure');
p();
p(`${epics.length} epics and ${features.length} feature issues map this inventory onto a build plan. Phases: 0 foundations (${metrics.backlog.byPhase[0]}), 1 core parity (${metrics.backlog.byPhase[1]}), 2 advanced audit and UX (${metrics.backlog.byPhase[2]}), 3 integrations and AI (${metrics.backlog.byPhase[3]}), 4 hardening and 1.0 (${metrics.backlog.byPhase[4]}).`);
p();
p('| Epic | Features | Phase |');
p('| --- | --- | --- |');
for (const e of epics) p(`| ${e.id} ${e.title} | ${features.filter((f) => f.epic === e.id).length} | ${e.phase} |`);
p();
p('The full matrix, with size, priority and dependencies, is in [parity-matrix.csv](reference/parity-matrix.csv).');
p();
p('## 16. Known gaps and open questions');
p();
p('- The CLI refuses to crawl without a licence file, so no end-to-end reference crawl output could be captured on this machine. Crawl-data comparisons must be recorded on a licensed install before parity testing begins.');
p('- Java identifiers in most packages are obfuscated (`id...`). Structure comes from package names, preserved semantic classes, resource bundles and runtime state, not from decompiled logic. Exact algorithms (e.g. readability formula constants, normalisation edge cases) remain to be characterised behaviourally.');
p('- The exact MCP tool catalogue is only visible at runtime in the app; it must be enumerated on a licensed install.');
p('- Internal database schema (crawl file format) could not be inspected without producing a crawl; this is the first task of the parity harness.');
p('- Licensing/business model is intentionally out of scope: Coqui is open source and will not implement licence enforcement, update nagging or usage telemetry.');

fs.writeFileSync(path.join(DOCS, 'feature-inventory.md'), md.join('\n') + '\n');

console.log(JSON.stringify(metrics, null, 2));
console.log(`wrote ${features.length} features, ${epics.length} epics`);

