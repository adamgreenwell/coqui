#!/usr/bin/env node
// Documentation consistency check.
//
// Reads counts and identifiers from the Rust sources and verifies that the
// hand-maintained documentation still matches them:
//
//   - every relative Markdown link resolves
//   - no literal "\n" sequences outside code fences
//   - static audit rule counts (prose, heading and table rows)
//   - schema version claims
//   - report kinds in docs/reports.md
//   - export families in docs/cli.md and docs/exports.md
//   - README/status backlog and catalogue counts agree with each other
//   - the versioning policy names the workspace version
//
// Run: node tools/docs/check.mjs

import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '../..');
const failures = [];
const fail = (message) => failures.push(message);
const read = (relative) => fs.readFileSync(path.join(ROOT, relative), 'utf8');
const exists = (absolute) => fs.existsSync(absolute);
const plain = (text) => text.replace(/\*/g, '');

function sliceBetween(text, start, end) {
  const first = text.indexOf(start);
  if (first < 0) return '';
  const last = text.indexOf(end, first + start.length);
  return text.slice(first, last < 0 ? undefined : last);
}

function sameSet(label, actual, expected) {
  const left = [...new Set(actual)].sort();
  const right = [...new Set(expected)].sort();
  const missing = right.filter((value) => !left.includes(value));
  const extra = left.filter((value) => !right.includes(value));
  if (missing.length || extra.length) {
    fail(`${label}: missing [${missing.join(', ')}], unexpected [${extra.join(', ')}]`);
  }
}

// ---------- source of truth ----------

const auditSource = read('crates/coqui-audit/src/lib.rs');
const ruleIds = [...sliceBetween(auditSource, 'pub const RULES', '];').matchAll(/id:\s*"([^"]+)"/g)].map(
  (match) => match[1],
);

const storeSource = read('crates/coqui-store/src/lib.rs');
const schemaVersion = Number(storeSource.match(/pub const SCHEMA_VERSION: i32 = (\d+);/)?.[1] ?? 0);

const reportSource = read('crates/coqui-report/src/lib.rs');
const reportKinds = [
  ...sliceBetween(reportSource, 'pub const REPORTS', '];').matchAll(/\(\s*"([a-z-]+)",\s*"([^"]+)",/g),
].map((match) => match[1]);

const coreSource = read('crates/coqui-core/src/lib.rs');
const catalogIds = [
  ...sliceBetween(coreSource, 'pub fn export_catalog', '#[cfg(test)]').matchAll(
    /entry\(\s*"([^"]+)",/g,
  ),
].map((match) => match[1]);

const cliSource = read('crates/coqui-cli/src/main.rs');
const familyUsage = cliSource.match(/--family ([a-z0-9|\-]+)\]/)?.[1] ?? '';
const cliFamilies = familyUsage.split('|').filter(Boolean);

const workspaceVersion = read('Cargo.toml').match(
  /\[workspace\.package\][\s\S]*?version = "([^"]+)"/,
)?.[1];

if (!ruleIds.length) fail('could not read RULES from coqui-audit');
if (!schemaVersion) fail('could not read SCHEMA_VERSION from coqui-store');
if (!reportKinds.length) fail('could not read REPORTS from coqui-report');
if (!catalogIds.length) fail('could not read export_catalog from coqui-core');
if (!cliFamilies.length) fail('could not read the --family list from coqui-cli usage');
if (!workspaceVersion) fail('could not read the workspace version from Cargo.toml');

// ---------- Markdown inventory ----------

function walk(directory, out = []) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (['node_modules', '.git', 'target'].includes(entry.name)) continue;
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) walk(absolute, out);
    else if (entry.name.endsWith('.md')) out.push(absolute);
  }
  return out;
}

const markdownFiles = [
  path.join(ROOT, 'README.md'),
  path.join(ROOT, 'CHANGELOG.md'),
  ...walk(path.join(ROOT, 'docs')),
].filter(exists);

function stripCodeFences(text) {
  const output = [];
  let fenced = false;
  for (const line of text.split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) {
      fenced = !fenced;
      output.push('');
      continue;
    }
    output.push(fenced ? '' : line);
  }
  return output.join('\n');
}

for (const file of markdownFiles) {
  const relative = path.relative(ROOT, file);
  const text = fs.readFileSync(file, 'utf8');
  const prose = stripCodeFences(text);

  for (const match of prose.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    let target = match[1].trim().replace(/^<|>$/g, '');
    if (/^(https?:|mailto:|#|\/)/i.test(target)) continue;
    target = target.split('#')[0].split('?')[0].trim();
    if (!target) continue;
    const resolved = path.resolve(path.dirname(file), decodeURI(target));
    if (!exists(resolved)) fail(`${relative}: broken link -> ${target}`);
  }

  if (prose.includes('\\n')) {
    const line = prose.split('\n').findIndex((entry) => entry.includes('\\n')) + 1;
    fail(`${relative}: literal \\n outside a code fence (line ${line})`);
  }
}

// ---------- audit rule counts ----------

const auditDoc = read('docs/audit-rules.md');
const headingCount = auditDoc.match(/## Static rules \((\d+)\)/)?.[1];
if (Number(headingCount) !== ruleIds.length) {
  fail(`docs/audit-rules.md: heading says ${headingCount ?? '?'} rules, code has ${ruleIds.length}`);
}
const ruleTable = sliceBetween(auditDoc, '## Static rules', '## Enumerated axe rules');
const tableRows = (ruleTable.match(/^\| `[a-z0-9_]+` \|/gm) ?? []).length;
if (tableRows !== ruleIds.length) {
  fail(`docs/audit-rules.md: table has ${tableRows} rows, code has ${ruleIds.length} rules`);
}

const thresholdTable = sliceBetween(auditDoc, '## Thresholds and triggers', '## Reference mapping');
const documentedTriggers = [...thresholdTable.matchAll(/^\| `([a-z0-9_*]+)` \|/gm)].map(
  (match) => match[1],
);
for (const rule of ruleIds) {
  if (rule === 'accessibility_violations') continue;
  if (!documentedTriggers.includes(rule)) {
    fail(`docs/audit-rules.md: no threshold entry for rule ${rule}`);
  }
}

for (const relative of [
  'README.md',
  'CHANGELOG.md',
  'docs/status.md',
  'docs/handover.md',
  'docs/dev-setup.md',
  'docs/audit-rules.md',
]) {
  for (const match of read(relative).matchAll(/(\d+)\s+(?:static|audit)\s+rules?/gi)) {
    if (Number(match[1]) !== ruleIds.length) {
      fail(`${relative}: says ${match[1]} rules, code has ${ruleIds.length}`);
    }
  }
}

// ---------- schema version ----------

for (const relative of [
  'README.md',
  'CHANGELOG.md',
  'docs/status.md',
  'docs/handover.md',
  'docs/dev-setup.md',
  'docs/versioning.md',
]) {
  const text = read(relative);
  const claims = [
    ...text.matchAll(/schema\s+version\s*\|\s*(\d+)/gi),
    ...text.matchAll(/schema[^\n]{0,24}?\bv(\d+)\b/gi),
    ...text.matchAll(/current version is \*\*(\d+)\*\*/gi),
  ].map((match) => Number(match[1]));
  for (const claim of claims) {
    if (claim !== schemaVersion) {
      fail(`${relative}: schema/version claim ${claim}, code has ${schemaVersion}`);
    }
  }
}

// ---------- report kinds ----------

const reportsDoc = read('docs/reports.md');
const reportTable = sliceBetween(reportsDoc, '## Catalogue', '## Options');
const documentedReports = [...reportTable.matchAll(/^\| `([a-z-]+)` \|/gm)].map(
  (match) => match[1],
);
sameSet('docs/reports.md catalogue', documentedReports, reportKinds);

for (const relative of ['README.md', 'docs/status.md', 'docs/handover.md']) {
  for (const match of read(relative).matchAll(/(\d+)\s+(?:HTML\/PDF )?report kinds/gi)) {
    if (Number(match[1]) !== reportKinds.length) {
      fail(`${relative}: says ${match[1]} report kinds, code has ${reportKinds.length}`);
    }
  }
}

// ---------- export families ----------

const cliDoc = read('docs/cli.md');
const documentedFamilies = [
  ...(cliDoc.match(/^Families: (.+)$/m)?.[1] ?? '').matchAll(/`([a-z0-9-]+)`/g),
].map((match) => match[1]);
sameSet('docs/cli.md export families', documentedFamilies, [...cliFamilies, 'all']);

const exportsDoc = read('docs/exports.md');
const familyTable = sliceBetween(exportsDoc, '## CLI and API families', '## App catalogue');
const documentedFamilyRows = [...familyTable.matchAll(/^\| `([a-z0-9-]+)` \|/gm)].map(
  (match) => match[1],
);
sameSet('docs/exports.md family table', documentedFamilyRows, cliFamilies);

for (const relative of ['README.md', 'docs/status.md']) {
  for (const match of read(relative).matchAll(/(\d+)\s+export families/gi)) {
    if (Number(match[1]) !== cliFamilies.length) {
      fail(`${relative}: says ${match[1]} export families, code has ${cliFamilies.length}`);
    }
  }
}

if (catalogIds.length && !exportsDoc.includes(`${catalogIds.length}-entry`)) {
  fail(`docs/exports.md: catalogue count does not say ${catalogIds.length}-entry`);
}

// ---------- backlog consistency ----------

const readme = read('README.md');
const status = read('docs/status.md');

const readmeBacklog = plain(readme).match(
  /(\d+)\s+issues\s*\((\d+) epics \+ (\d+) features\)/i,
);
const statusBacklog = plain(status).match(
  /\|\s*Issues\s*\|\s*(\d+)\s*\((\d+) epics \+ (\d+) features\)/i,
);
if (!readmeBacklog || !statusBacklog) {
  fail('README/status: could not read the backlog summary');
} else if (readmeBacklog.slice(1).join('/') !== statusBacklog.slice(1).join('/')) {
  fail(
    `README backlog ${readmeBacklog.slice(1).join('/')} differs from status ${statusBacklog
      .slice(1)
      .join('/')}`,
  );
}

const readmeOpenClosed = plain(readme).match(/(\d+) open and (\d+) closed/i);
const statusOpenClosed = status.match(/\|\s*Open \/ closed\s*\|\s*(\d+)\s*\/\s*(\d+)\s*\|/i);
if (!readmeOpenClosed || !statusOpenClosed) {
  fail('README/status: could not read the open/closed issue counts');
} else if (readmeOpenClosed.slice(1).join('/') !== statusOpenClosed.slice(1).join('/')) {
  fail(
    `README open/closed ${readmeOpenClosed.slice(1).join('/')} differs from status ${statusOpenClosed
      .slice(1)
      .join('/')}`,
  );
}

// ---------- version ----------

const versioningDoc = read('docs/versioning.md');
const versionClaim = versioningDoc.match(/current version is \*\*([^*]+)\*\*/i)?.[1];
if (versionClaim !== workspaceVersion) {
  fail(`docs/versioning.md: version ${versionClaim ?? '?'}, Cargo.toml has ${workspaceVersion}`);
}

// ---------- report ----------

console.log(
  `checked ${markdownFiles.length} markdown files against ${ruleIds.length} rules, ` +
    `${reportKinds.length} report kinds, ${cliFamilies.length} export families, ` +
    `${catalogIds.length} catalogue entries and schema v${schemaVersion}`,
);

if (failures.length) {
  console.error(`\n${failures.length} documentation issue(s):`);
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log('docs are consistent');
