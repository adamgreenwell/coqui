import fs from 'node:fs';
import { DatabaseSync } from 'node:sqlite';

const dbPath = process.argv[2];
const outPath = process.argv[3];
if (!dbPath || !outPath) {
  console.error('usage: node tools/parity/sf-crawl-db.mjs <sf-crawl.db> <out.csv>');
  process.exit(2);
}
const db = new DatabaseSync(dbPath, { readOnly: true });
const rows = db.prepare("select * from urls where is_internal = 1 order by encoded_url").all();
const escape = (value) => {
  const text = value === null || value === undefined ? '' : String(value);
  return /[",\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
};
const headers = ['url','status','title','meta_description','h1','word_count','response_time_ms','size_bytes','depth','content_type'];
const lines = [headers.join(',')];
for (const row of rows) {
  let meta = '';
  for (let index = 1; index <= 20; index += 1) {
    const name = row[`meta_name_${index}`];
    if (name && String(name).toLowerCase() === 'description') {
      meta = row[`meta_content_${index}`] || '';
      break;
    }
  }
  lines.push([
    row.encoded_url,
    row.response_code,
    row.title_1,
    meta,
    row.h1_1,
    row.word_count,
    row.response_time_ms,
    row.page_size,
    row.crawl_depth,
    row.content_type,
  ].map(escape).join(','));
}
fs.writeFileSync(outPath, lines.join('\n'));
console.log(`wrote ${outPath}: ${rows.length} internal URLs, ${headers.length} columns`);
