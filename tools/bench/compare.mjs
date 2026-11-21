#!/usr/bin/env node
// Compare two `coqui bench --json` results and fail on >10% regression.
// Usage: node tools/bench/compare.mjs --previous prev.json --current cur.json [--max-regression 0.10]

import fs from 'node:fs';

const args = {};
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index];
  if (key === '--previous') args.previous = process.argv[index + 1];
  else if (key === '--current') args.current = process.argv[index + 1];
  else if (key === '--max-regression') args.maxRegression = Number(process.argv[index + 1]);
  else if (key === '--out') args.out = process.argv[index + 1];
}
if (!args.current) {
  console.error('usage: node tools/bench/compare.mjs --previous prev.json --current cur.json');
  process.exit(2);
}
const maxRegression = Number.isFinite(args.maxRegression) ? args.maxRegression : 0.1;
const current = JSON.parse(fs.readFileSync(args.current, 'utf8'));
if (!args.previous || !fs.existsSync(args.previous)) {
  console.log('no previous benchmark result; skipping regression check');
  process.exit(0);
}
const previous = JSON.parse(fs.readFileSync(args.previous, 'utf8'));
if (current.urls !== previous.urls) {
  console.log(`URL counts differ (${previous.urls} -> ${current.urls}); skipping regression check`);
  process.exit(0);
}

const currentPerUrl = current.db_bytes / Math.max(current.urls, 1);
const previousPerUrl = previous.db_bytes / Math.max(previous.urls, 1);
const metrics = [
  { name: 'urls_per_second', before: previous.urls_per_second, after: current.urls_per_second, direction: 'higher' },
  { name: 'query_ms', before: previous.query_ms, after: current.query_ms, direction: 'lower' },
  { name: 'bytes_per_url', before: previousPerUrl, after: currentPerUrl, direction: 'lower' },
];

let failed = false;
const lines = [
  '# Benchmark trend',
  '',
  `- URLs: ${current.urls}`,
  '',
  '| Metric | Previous | Current | Change | Status |',
  '| --- | --- | --- | --- | --- |',
];
for (const metric of metrics) {
  if (metric.before <= 0) continue;
  const change = (metric.after - metric.before) / metric.before;
  const regression = metric.direction === 'higher' ? -change : change;
  const status = regression > maxRegression ? 'FAIL' : 'ok';
  console.log(
    `${status} ${metric.name}: ${metric.before.toFixed(0)} -> ${metric.after.toFixed(0)} (${(change * 100).toFixed(1)}%)`,
  );
  lines.push(
    `| ${metric.name} | ${metric.before.toFixed(0)} | ${metric.after.toFixed(0)} | ${(change * 100).toFixed(1)}% | ${status} |`,
  );
  if (regression > maxRegression) failed = true;
}
if (args.out) fs.writeFileSync(args.out, `${lines.join('\n')}\n`);
if (failed) {
  console.error(`regression exceeds ${(maxRegression * 100).toFixed(0)}%`);
  process.exit(3);
}
