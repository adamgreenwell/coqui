#!/usr/bin/env node
// Compare Screaming Frog export CSVs with Coqui CSV exports.
// Usage: node tools/parity/compare-exports.mjs --coqui DIR --reference DIR [--out report.md]

import fs from 'node:fs';
import path from 'node:path';

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === '--coqui') args.coqui = argv[++index];
    else if (key === '--reference') args.reference = argv[++index];
    else if (key === '--out') args.out = argv[++index];
    else if (key === '--fail-under-ratio') args.failUnder = Number(argv[++index]);
    else if (key === '--max-mismatches') args.maxMismatches = Number(argv[++index]);
    else if (key === '--tolerance') args.tolerance = argv[++index];
    else if (key === '--compare-values-for') args.compareValuesFor = argv[++index];
    else if (key === '--require-counterparts') args.requireCounterparts = true;
  }
  return args;
}

function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (quoted) {
      if (char === '"') {
        if (text[index + 1] === '"') {
          field += '"';
          index += 1;
        } else {
          quoted = false;
        }
      } else {
        field += char;
      }
    } else if (char === '"') {
      quoted = true;
    } else if (char === ',') {
      row.push(field);
      field = '';
    } else if (char === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
    } else if (char !== '\r') {
      field += char;
    }
  }
  if (field.length > 0 || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows.filter((entry) => entry.some((value) => value !== ''));
}

function readCsv(file) {
  const rows = parseCsv(fs.readFileSync(file, 'utf8'));
  if (rows.length === 0) return { headers: [], rows: [] };
  return { headers: rows[0], rows: rows.slice(1) };
}

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.name.toLowerCase().endsWith('.csv')) out.push(full);
  }
  return out;
}

const URL_COLUMNS = ['address', 'url', 'url address', 'address (url)', 'page url'];
function urlColumn(headers) {
  const lower = headers.map((header) => header.trim().toLowerCase());
  for (const candidate of URL_COLUMNS) {
    const index = lower.indexOf(candidate);
    if (index >= 0) return index;
  }
  const fuzzy = lower.findIndex((header) => header === 'url' || header.startsWith('url '));
  return fuzzy;
}

const SYNONYMS = {
  address: 'url',
  'status code': 'status',
  'page title': 'title',
  'meta description': 'meta_description',
  'meta description 1': 'meta_description',
  'h1-1': 'h1',
  'h1 1': 'h1',
  'word count': 'word_count',
  'response time': 'response_time_ms',
};

function normalizeHeader(header) {
  const lower = header.trim().toLowerCase();
  return SYNONYMS[lower] ?? lower;
}

function sample(list, count = 3) {
  return list.slice(0, count).join(', ');
}

const args = parseArgs(process.argv.slice(2));
if (!args.coqui || !args.reference) {
  console.error('usage: node tools/parity/compare-exports.mjs --coqui DIR --reference DIR [--out report.md]');
  process.exit(2);
}

const coquiUrlsPath = path.join(args.coqui, 'urls.csv');
if (!fs.existsSync(coquiUrlsPath)) {
  console.error(`missing ${coquiUrlsPath}`);
  process.exit(2);
}
const coquiUrls = readCsv(coquiUrlsPath);
const coquiUrlIndex = urlColumn(coquiUrls.headers);
if (coquiUrlIndex < 0) {
  console.error('coqui urls.csv has no URL column');
  process.exit(2);
}
const coquiSet = new Set(coquiUrls.rows.map((row) => row[coquiUrlIndex]));
const coquiHeaders = new Set(coquiUrls.headers.map((header) => normalizeHeader(header)));

if (!fs.existsSync(args.reference) || !fs.statSync(args.reference).isDirectory()) {
  console.error(
    `reference folder not found: ${args.reference}\n` +
      'Create it and export Screaming Frog CSVs into it, for example:\n' +
      `  New-Item -ItemType Directory -Force ${args.reference}`,
  );
  process.exit(2);
}
const referenceFiles = walk(args.reference);
if (referenceFiles.length === 0) {
  console.error(
    `no CSV files under ${args.reference}\n` +
      'Export the tabs/reports as CSV (Bulk Export) and retry.',
  );
  process.exit(2);
}
const lines = [];
lines.push('# Parity export comparison');
lines.push('');
lines.push(`- Coqui: \`${args.coqui}\` (${coquiUrls.rows.length} URLs, ${coquiUrls.headers.length} columns)`);
lines.push(`- Reference: \`${args.reference}\` (${referenceFiles.length} CSV files)`);
lines.push('');

const tolerance = args.tolerance
  ? JSON.parse(fs.readFileSync(args.tolerance, 'utf8'))
  : {};
const toleranceFor = (header) =>
  tolerance[header] ?? tolerance[normalizeHeader(header)] ?? null;

function rowsMatch(a, b, headers) {
  for (let index = 0; index < headers.length; index += 1) {
    const header = headers[index];
    const left = a[index] ?? '';
    const right = b[index] ?? '';
    const rule = toleranceFor(header);
    if (rule?.type === 'ignore') continue;
    if (rule?.type === 'numeric') {
      const hasBoth = left.trim() !== '' && right.trim() !== '';
      const x = Number(left);
      const y = Number(right);
      if (
        hasBoth &&
        Number.isFinite(x) &&
        Number.isFinite(y) &&
        Math.abs(x - y) <= (rule.absolute ?? 0)
      ) {
        continue;
      }
    }
    if (left !== right) return false;
  }
  return true;
}

let matchedFiles = 0;
let totalRefUrls = 0;
let totalOverlap = 0;
let totalMismatches = 0;
let missingCounterparts = 0;
const coquiHeaderIndex = new Map(
  coquiUrls.headers.map((header, index) => [normalizeHeader(header), index]),
);
const coquiByUrl = new Map();
if (coquiUrlIndex >= 0) {
  for (const row of coquiUrls.rows) coquiByUrl.set(row[coquiUrlIndex], row);
}
const coquiFiles = new Map();
for (const file of walk(args.coqui)) {
  const base = path.basename(file);
  const { headers, rows } = readCsv(file);
  const index = urlColumn(headers);
  const byUrl = new Map();
  if (index >= 0) for (const row of rows) byUrl.set(row[index], row);
  coquiFiles.set(base, {
    headers,
    rows,
    index,
    byUrl,
    headerIndex: new Map(headers.map((header, i) => [normalizeHeader(header), i])),
  });
}
for (const file of referenceFiles) {
  const relative = path.relative(args.reference, file);
  const { headers, rows } = readCsv(file);
  const urlIndex = urlColumn(headers);
  lines.push(`## ${relative}`);
  lines.push('');
  lines.push(`- Rows: ${rows.length}; columns: ${headers.length}`);
  if (urlIndex < 0) {
    lines.push('- URL column: not detected');
  } else {
    const urls = rows.map((row) => row[urlIndex]).filter(Boolean);
    const overlap = urls.filter((url) => coquiSet.has(url)).length;
    totalRefUrls += urls.length;
    totalOverlap += overlap;
    lines.push(`- URL column: \`${headers[urlIndex]}\`; overlap with Coqui: ${overlap}/${urls.length}`);
    if (urls.length > 0) {
      const missing = urls.filter((url) => !coquiSet.has(url));
      if (missing.length > 0) lines.push(`- Missing from Coqui (sample): ${sample(missing)}`);
      matchedFiles += 1;
    }
  }
  const base = path.basename(relative);
  const counterpart = coquiFiles.get(base) ?? null;
  if (args.requireCounterparts && !counterpart) {
    missingCounterparts += 1;
    lines.push(`- Missing Coqui counterpart: \`${base}\``);
  }
  const shared = headers.filter((header) =>
    counterpart
      ? counterpart.headerIndex.has(normalizeHeader(header))
      : coquiHeaders.has(normalizeHeader(header)),
  );
  const valueFiles = args.compareValuesFor
    ? args.compareValuesFor
        .split(',')
        .map((name) => name.trim())
        .filter(Boolean)
    : null;
  const refUrls = urlIndex >= 0 ? rows.map((row) => row[urlIndex]) : [];
  const counterpartUrls =
    counterpart && counterpart.index >= 0
      ? counterpart.rows.map((row) => row[counterpart.index])
      : [];
  const repeatedUrls =
    refUrls.length !== new Set(refUrls).size ||
    counterpartUrls.length !== new Set(counterpartUrls).size;
  const compareValues =
    !repeatedUrls &&
    urlIndex >= 0 &&
    (counterpart ? counterpart.index >= 0 : coquiUrlIndex >= 0) &&
    (valueFiles
      ? valueFiles.includes(base)
      : Boolean(counterpart) || /urls\.csv$/i.test(base));
  if (counterpart && (urlIndex < 0 || counterpart.index < 0 || repeatedUrls)) {
    const refIndex = new Map(
      headers.map((header, index) => [normalizeHeader(header), index]),
    );
    const project = (row, headerIndex) =>
      headers.map((header) => {
        const index = headerIndex.get(normalizeHeader(header));
        return index === undefined ? '' : (row[index] ?? '');
      });
    const others = counterpart.rows.map((row) => project(row, counterpart.headerIndex));
    const used = new Set();
    let fileMismatches = 0;
    for (const row of rows) {
      const left = project(row, refIndex);
      const match = others.findIndex(
        (right, index) => !used.has(index) && rowsMatch(left, right, headers),
      );
      if (match >= 0) used.add(match);
      else fileMismatches += 1;
    }
    fileMismatches += others.length - used.size;
    totalMismatches += fileMismatches;
    lines.push(`- Row mismatches: ${fileMismatches}`);
  }
  if (compareValues) {
    const otherByUrl = counterpart ? counterpart.byUrl : coquiByUrl;
    const otherHeaderIndex = counterpart ? counterpart.headerIndex : coquiHeaderIndex;
    let fileMismatches = 0;
    for (const row of rows) {
      const url = row[urlIndex];
      const other = otherByUrl.get(url);
      if (!other) continue;
      for (const header of shared) {
        const a = row[headers.indexOf(header)] ?? '';
        const b = other[otherHeaderIndex.get(normalizeHeader(header))] ?? '';
        const rule = toleranceFor(header);
        if (rule?.type === 'ignore') continue;
        if (rule?.type === 'numeric') {
          const hasBoth = a.trim() !== '' && b.trim() !== '';
          const x = Number(a);
          const y = Number(b);
          if (
            hasBoth &&
            Number.isFinite(x) &&
            Number.isFinite(y) &&
            Math.abs(x - y) <= (rule.absolute ?? 0)
          ) {
            continue;
          }
        }
        if (a !== b) fileMismatches += 1;
      }
    }
    const refUrlSet = new Set(rows.map((row) => row[urlIndex]));
    const otherUrlIndex = counterpart ? counterpart.index : coquiUrlIndex;
    const otherUrlSet = new Set(
      (counterpart ? counterpart.rows : coquiUrls.rows).map((row) => row[otherUrlIndex]),
    );
    fileMismatches += [...refUrlSet].filter((url) => !otherUrlSet.has(url)).length;
    fileMismatches += [...otherUrlSet].filter((url) => !refUrlSet.has(url)).length;
    totalMismatches += fileMismatches;
    lines.push(`- Shared-column value mismatches: ${fileMismatches}`);
  }
  const only = headers.filter((header) => !coquiHeaders.has(normalizeHeader(header)));
  lines.push(`- Headers also in Coqui urls.csv: ${shared.length}`);
  if (only.length > 0) lines.push(`- Reference-only headers (sample): ${sample(only, 8)}`);
  lines.push('');
}

lines.push('## Summary');
lines.push('');
lines.push(`- Reference files with a detected URL column: ${matchedFiles}/${referenceFiles.length}`);
lines.push(`- Coqui URL rows: ${coquiUrls.rows.length}`);
lines.push(`- URL overlap across reference files: ${totalOverlap}/${totalRefUrls}`);
lines.push(`- Shared-column value mismatches: ${totalMismatches}`);
lines.push('');
lines.push('Next: map each reference tab to a Coqui export family, then compare row-level values for the shared columns.');

const report = lines.join('\n');
if (args.requireCounterparts && missingCounterparts > 0) {
  console.error(`${missingCounterparts} reference files have no Coqui counterpart`);
  process.exit(3);
}
if (args.failUnder && totalRefUrls === 0) {
  console.error('no reference URLs with a detected URL column; cannot enforce overlap');
  process.exit(3);
}
if (args.failUnder && totalRefUrls > 0 && totalOverlap / totalRefUrls < args.failUnder) {
  console.error(`URL overlap ${totalOverlap}/${totalRefUrls} is below ${args.failUnder}`);
  process.exit(3);
}
if (Number.isFinite(args.maxMismatches) && totalMismatches > args.maxMismatches) {
  console.error(`${totalMismatches} shared-column mismatches exceed ${args.maxMismatches}`);
  process.exit(3);
}
if (args.out) {
  fs.mkdirSync(path.dirname(path.resolve(args.out)), { recursive: true });
  fs.writeFileSync(args.out, report);
  console.log(`wrote ${args.out}`);
} else {
  console.log(report);
}

