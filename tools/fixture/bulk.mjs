// Bulk fixture for scale testing: a sitemap with N URLs and minimal pages.
// Run: node tools/fixture/bulk.mjs 5000 4180
import http from 'node:http';

const count = Number(process.argv[2] ?? 5000);
const port = Number(process.argv[3] ?? 4180);
const origin = `http://127.0.0.1:${port}`;

const server = http.createServer((request, response) => {
  const url = new URL(request.url ?? '/', origin);
  if (url.pathname === '/robots.txt') {
    response.writeHead(200, { 'content-type': 'text/plain' });
    response.end('User-agent: *\nAllow: /\n');
    return;
  }
  if (url.pathname === '/sitemap.xml') {
    const body = [];
    for (let i = 0; i < count; i += 1) {
      body.push(`<url><loc>${origin}/p/${i}</loc></url>`);
    }
    response.writeHead(200, { 'content-type': 'application/xml' });
    response.end(
      `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${body.join('')}</urlset>`,
    );
    return;
  }
  const match = /^\/p\/(\d+)$/.exec(url.pathname);
  if (match) {
    const index = Number(match[1]);
    const next = (index + 1) % count;
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(
      `<!doctype html><html><head><title>Page ${index}</title><meta name="description" content="Bulk fixture page ${index}"><link rel="canonical" href="${origin}/p/${index}"></head><body><h1>Page ${index}</h1><p>${'word '.repeat(220)}</p><a href="/p/${next}">Next</a></body></html>`,
    );
    return;
  }
  response.writeHead(404, { 'content-type': 'text/html' });
  response.end('<html><body>not found</body></html>');
});

server.listen(port, '127.0.0.1', () => {
  console.log(`bulk fixture with ${count} URLs on ${origin}`);
});
