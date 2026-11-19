// Repeatable crawl benchmark against the local bulk fixture.
//
// node tools/bench/run.mjs --urls 2000 --port 4190
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 && args[index + 1] ? args[index + 1] : fallback;
};

const urls = Number(option('urls', '2000'));
const port = Number(option('port', '4190'));
const render = args.includes('--render');
const stamp = new Date().toISOString().replace(/[:.]/g, '-');
const out = path.join('bench', stamp);

fs.mkdirSync(out, { recursive: true });

console.log(`building CLI…`);
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
  for (let attempt = 0; attempt < 100; attempt += 1) {
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

try {
  await waitForFixture();
  const binary = path.join('target', 'debug', process.platform === 'win32' ? 'coqui.exe' : 'coqui');
  const started = process.hrtime.bigint();
  const crawlArgs = [
    'crawl',
    'sitemap',
    `${origin}/sitemap.xml`,
    '--max-urls',
    String(urls),
    '--out',
    out,
  ];
  if (render) {
    crawlArgs.push('--render', '--screenshots', '--a11y', '--vitals');
  }
  const crawl = spawnSync(
    binary,
    crawlArgs,
    { encoding: 'utf8', shell: false },
  );
  const seconds = Number(process.hrtime.bigint() - started) / 1e9;
  if (crawl.status !== 0) {
    console.error(crawl.stderr || crawl.stdout);
    process.exit(crawl.status ?? 1);
  }
  const parse = /done: (\d+) crawled, (\d+) discovered, (\d+) issues/.exec(crawl.stdout);
  const dbBytes = fs.statSync(path.join(out, 'coqui.db')).size;
  const csvPath = path.join(out, 'urls.csv');
  const csvBytes = fs.existsSync(csvPath) ? fs.statSync(csvPath).size : 0;
  const result = {
    timestamp: new Date().toISOString(),
    fixture: { urls, port, render },
    seconds: Number(seconds.toFixed(3)),
    urlsPerSecond: Number((urls / seconds).toFixed(1)),
    crawled: parse ? Number(parse[1]) : null,
    discovered: parse ? Number(parse[2]) : null,
    issues: parse ? Number(parse[3]) : null,
    dbBytes,
    csvBytes,
    node: process.version,
    platform: process.platform,
  };
  fs.writeFileSync(path.join(out, 'result.json'), `${JSON.stringify(result, null, 2)}\n`);
  fs.writeFileSync(
    path.join('bench', 'latest.json'),
    `${JSON.stringify(result, null, 2)}\n`,
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  fixture.kill();
}

