// Fetch pinned Node and Lighthouse sidecars with checksum verification.
//
// node tools/sidecar/fetch.mjs
//   --node 22.14.0 --lighthouse 12.8.2 --out src-tauri/sidecars
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 && args[index + 1] ? args[index + 1] : fallback;
};

const nodeVersion = option('node', '22.14.0');
const lighthouseVersion = option('lighthouse', '12.8.2');
const out = path.resolve(option('out', 'src-tauri/sidecars'));
const platform = process.platform === 'win32' ? 'win' : process.platform;
const arch = process.arch;
fs.mkdirSync(out, { recursive: true });

const download = async (url) => {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${response.status} for ${url}`);
  return Buffer.from(await response.arrayBuffer());
};

const sha256 = (buffer) => crypto.createHash('sha256').update(buffer).digest('hex');
const sha512 = (buffer) => crypto.createHash('sha512').update(buffer).digest('base64');

console.log(`node v${nodeVersion} for ${platform}-${arch}`);
const base = `https://nodejs.org/dist/v${nodeVersion}`;
const sums = (await download(`${base}/SHASUMS256.txt`)).toString('utf8');
const relative =
  platform === 'win'
    ? `${platform}-${arch}/node.exe`
    : `node-v${nodeVersion}-${platform}-${arch}.tar.gz`;
const expectedLine = sums
  .split('\n')
  .map((line) => line.trim().split(/\s+/))
  .find((parts) => parts[1] === relative || parts[1] === `./${relative}`);
if (!expectedLine) throw new Error(`checksum not found for ${relative}`);
const nodeBytes = await download(`${base}/${relative}`);
const nodeDigest = sha256(nodeBytes);
if (nodeDigest !== expectedLine[0]) {
  throw new Error(`node checksum mismatch: ${nodeDigest} != ${expectedLine[0]}`);
}
const nodeBinary = path.join(out, platform === 'win' ? 'node.exe' : 'node');
if (platform === 'win') {
  fs.writeFileSync(nodeBinary, nodeBytes);
} else {
  const archive = path.join(out, relative);
  fs.writeFileSync(archive, nodeBytes);
  const extract = path.join(out, '.node-extract');
  fs.mkdirSync(extract, { recursive: true });
  const tar = spawnSync('tar', ['-xzf', archive, '-C', extract], { stdio: 'inherit' });
  if (tar.status !== 0) throw new Error('node tarball extraction failed');
  fs.copyFileSync(path.join(extract, `node-v${nodeVersion}-${platform}-${arch}`, 'bin', 'node'), nodeBinary);
  fs.rmSync(archive, { force: true });
  fs.rmSync(extract, { recursive: true, force: true });
}
fs.chmodSync(nodeBinary, 0o755);

console.log(`lighthouse v${lighthouseVersion}`);
const lighthouseDir = path.join(out, 'lighthouse');
fs.rmSync(lighthouseDir, { recursive: true, force: true });
fs.mkdirSync(lighthouseDir, { recursive: true });
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
const install = spawnSync(
  npm,
  [
    'install',
    '--prefix',
    lighthouseDir,
    '--omit=dev',
    '--ignore-scripts',
    '--no-audit',
    '--no-fund',
    `lighthouse@${lighthouseVersion}`,
  ],
  { stdio: 'inherit', shell: process.platform === 'win32' },
);
if (install.status !== 0) throw new Error('lighthouse install failed');
const packagePath = path.join(lighthouseDir, 'node_modules', 'lighthouse', 'package.json');
const installed = JSON.parse(fs.readFileSync(packagePath, 'utf8'));
if (installed.version !== lighthouseVersion) {
  throw new Error(`lighthouse version ${installed.version} != ${lighthouseVersion}`);
}
let integrity = null;
const lockPath = path.join(lighthouseDir, 'package-lock.json');
if (fs.existsSync(lockPath)) {
  const lock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
  integrity = lock.packages?.['node_modules/lighthouse']?.integrity ?? null;
}
if (!fs.existsSync(path.join(lighthouseDir, 'node_modules', 'lighthouse', 'cli', 'index.js'))) {
  throw new Error('lighthouse cli entry missing after install');
}

const manifest = {
  fetchedAt: new Date().toISOString(),
  platform,
  arch,
  node: { version: nodeVersion, binary: path.basename(nodeBinary), sha256: nodeDigest },
  lighthouse: { version: lighthouseVersion, integrity },
};
fs.writeFileSync(path.join(out, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`sidecars written to ${out}`);

