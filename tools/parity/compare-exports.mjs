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

let matchedFiles = 0;
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
    lines.push(`- URL column: \`${headers[urlIndex]}\`; overlap with Coqui: ${overlap}/${urls.length}`);
    if (urls.length > 0) {
      const missing = urls.filter((url) => !coquiSet.has(url));
      if (missing.length > 0) lines.push(`- Missing from Coqui (sample): ${sample(missing)}`);
      matchedFiles += 1;
    }
  }
  const shared = headers.filter((header) => coquiHeaders.has(normalizeHeader(header)));
  const only = headers.filter((header) => !coquiHeaders.has(normalizeHeader(header)));
  lines.push(`- Headers also in Coqui urls.csv: ${shared.length}`);
  if (only.length > 0) lines.push(`- Reference-only headers (sample): ${sample(only, 8)}`);
  lines.push('');
}

lines.push('## Summary');
lines.push('');
lines.push(`- Reference files with a detected URL column: ${matchedFiles}/${referenceFiles.length}`);
lines.push(`- Coqui URL rows: ${coquiUrls.rows.length}`);
lines.push('');
lines.push('Next: map each reference tab to a Coqui export family, then compare row-level values for the shared columns.');

const report = lines.join('\n');
if (args.out) {
  fs.mkdirSync(path.dirname(path.resolve(args.out)), { recursive: true });
  fs.writeFileSync(args.out, report);
  console.log(`wrote ${args.out}`);
} else {
  console.log(report);
}
