#!/usr/bin/env node
// Compare internal link sets between two Coqui databases.
// Usage: node tools/parity/compare-links.mjs --left DB --right DB [--out report.md]

import fs from 'node:fs';
import { DatabaseSync } from 'node:sqlite';

const args = {};
for (let index = 0; index < process.argv.length - 2; index += 2) {
  const key = process.argv[index + 2];
  if (key === '--left') args.left = process.argv[index + 3];
  else if (key === '--right') args.right = process.argv[index + 3];
  else if (key === '--out') args.out = process.argv[index + 3];
}
if (!args.left || !args.right) {
  console.error('usage: node tools/parity/compare-links.mjs --left DB --right DB [--out report.md]');
  process.exit(2);
}

function load(dbPath) {
  const db = new DatabaseSync(dbPath, { readOnly: true });
  const links = new Set();
  const pages = new Set();
  const rows = db
    .prepare(
      `select f.url from_url, t.url to_url
       from links l
       join url_map f on f.id = l.from_id
       join url_map t on t.id = l.to_id
       join urls src on src.url = f.url
       join urls dst on dst.url = t.url
       where l.internal = 1
         and src.content_type like 'text/html%'
         and dst.content_type like 'text/html%'`,
    )
    .all();
  for (const row of rows) {
    links.add(`${row.from_url}\t${row.to_url}`);
    pages.add(row.from_url);
  }
  const urls = new Set(
    db.prepare("select url from urls where content_type like 'text/html%'").all().map((row) => row.url),
  );
  return { links, pages, urls, path: dbPath };
}

const left = load(args.left);
const right = load(args.right);
const sharedPages = [...left.pages].filter((page) => right.pages.has(page));
const sharedUrls = new Set([...left.urls].filter((url) => right.urls.has(url)));
let compared = 0;
let both = 0;
const onlyLeft = [];
const onlyRight = [];
for (const page of sharedPages) {
  const onPage = (link, set) => {
    const [from, to] = link.split('\t');
    return from === page && set.has(to);
  };
  const leftLinks = new Set([...left.links].filter((link) => onPage(link, sharedUrls)));
  const rightLinks = new Set([...right.links].filter((link) => onPage(link, sharedUrls)));
  compared += 1;
  for (const link of leftLinks) {
    if (rightLinks.has(link)) both += 1;
    else onlyLeft.push(link);
  }
  for (const link of rightLinks) {
    if (!leftLinks.has(link)) onlyRight.push(link);
  }
}

const lines = [
  '# Link set comparison',
  '',
  `- Left: \`${left.path}\` (${left.links.size} internal links, ${left.pages.size} source pages)`,
  `- Right: \`${right.path}\` (${right.links.size} internal links, ${right.pages.size} source pages)`,
  `- Shared source pages: ${sharedPages.length}`,
  '',
  `- Links in both: ${both}`,
  `- Links only in left: ${onlyLeft.length}`,
  `- Links only in right: ${onlyRight.length}`,
  '',
  '## Sample links only in left',
  '',
  ...onlyLeft.slice(0, 10).map((link) => `- ${link.replace('\t', ' -> ')}`),
  '',
  '## Sample links only in right',
  '',
  ...onlyRight.slice(0, 10).map((link) => `- ${link.replace('\t', ' -> ')}`),
];

const report = lines.join('\n');
if (args.out) {
  fs.writeFileSync(args.out, report);
  console.log(`wrote ${args.out}`);
} else {
  console.log(report);
}
