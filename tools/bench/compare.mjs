// Two-crawl comparison benchmark against the local bulk fixture.
// node tools/bench/compare.mjs --urls 10000 --port 4195
import { spawn, spawnSync } from 'node:child_process';
import { DatabaseSync } from 'node:sqlite';
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 && args[index + 1] ? args[index + 1] : fallback;
};

const urls = Number(option('urls', '10000'));
const port = Number(option('port', '4195'));
const stamp = new Date().toISOString().replace(/[:.]/g, '-');
const out = path.join('bench', `compare-${stamp}`);
fs.mkdirSync(out, { recursive: true });

console.log('building CLI…');
const build = spawnSync('cargo', ['build', '-q', '-p', 'coqui-cli'], {
  stdio: 'inherit',
  shell: process.platform === 'win32',
});
if (build.status !== 0) process.exit(build.status ?? 1);

console.log(`starting bulk fixture: ${urls} URLs on :${port}`);
const fixture = spawn(process.execPath, ['tools/fixture/bulk.mjs', String(urls), String(port)], {
  stdio: 'ignore',
});
const origin = `http://127.0.0.1:${port}`;
const waitForFixture = async () => {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    try {
      const response = await fetch(`${origin}/sitemap.xml`);
      if (response.ok) return;
    } catch {
      // retry
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('fixture did not start');
};

const binary = path.join('target', 'debug', process.platform === 'win32' ? 'coqui.exe' : 'coqui');
const crawl = () =>
  spawnSync(
    binary,
    ['crawl', 'sitemap', `${origin}/sitemap.xml`, '--max-urls', String(urls), '--out', out],
    { stdio: 'inherit' },
  );

try {
  await waitForFixture();
  const first = crawl();
  if (first.status !== 0) process.exit(first.status ?? 1);
  const second = crawl();
  if (second.status !== 0) process.exit(second.status ?? 1);

  const db = new DatabaseSync(path.join(out, 'coqui.db'));
  const ids = db
    .prepare('select id from crawls order by created_at asc')
    .all()
    .map((row) => row.id);
  console.log(`crawls: ${ids.join(', ')}`);

  const started = process.hrtime.bigint();
  const compare = spawnSync(binary, ['report', 'compare', '--with', ids[0], '--out', out], {
    stdio: 'inherit',
  });
  const elapsed = Number(process.hrtime.bigint() - started) / 1e6;
  if (compare.status !== 0) process.exit(compare.status ?? 1);
  console.log(`comparison of ${urls} URLs took ${elapsed.toFixed(0)} ms`);
} finally {
  fixture.kill();
}
