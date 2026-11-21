#!/usr/bin/env node
// Validate a `coqui bench --json` result against budgets.
// Usage: node tools/bench/check.mjs --input bench.json [--budgets tools/bench/budgets.json]

import fs from 'node:fs';

const args = {};
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index];
  if (key === '--input') args.input = process.argv[index + 1];
  else if (key === '--budgets') args.budgets = process.argv[index + 1];
}
if (!args.input) {
  console.error('usage: node tools/bench/check.mjs --input bench.json [--budgets budgets.json]');
  process.exit(2);
}

const result = JSON.parse(fs.readFileSync(args.input, 'utf8'));
const budgets = JSON.parse(
  fs.readFileSync(args.budgets ?? 'tools/bench/budgets.json', 'utf8'),
);
const perUrl = result.db_bytes / Math.max(result.urls, 1);

const checks = [
  {
    name: 'urls',
    actual: result.urls,
    limit: budgets.urls,
    ok: result.urls === budgets.urls,
    unit: '',
  },
  {
    name: 'urls_per_second',
    actual: result.urls_per_second,
    limit: budgets.min_urls_per_second,
    ok: result.urls_per_second >= budgets.min_urls_per_second,
    unit: '/s',
  },
  {
    name: 'query_ms',
    actual: result.query_ms,
    limit: budgets.max_query_ms,
    ok: result.query_ms <= budgets.max_query_ms,
    unit: 'ms',
  },
  {
    name: 'bytes_per_url',
    actual: Math.round(perUrl),
    limit: budgets.max_bytes_per_url,
    ok: perUrl <= budgets.max_bytes_per_url,
    unit: 'B',
  },
];

let failed = false;
for (const check of checks) {
  const status = check.ok ? 'ok' : 'FAIL';
  console.log(
    `${status} ${check.name}: ${check.actual}${check.unit} (limit ${check.limit}${check.unit})`,
  );
  if (!check.ok) failed = true;
}
if (failed) {
  console.error('benchmark exceeded budget');
  process.exit(3);
}
