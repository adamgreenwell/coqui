// Local demo site for M1 verification. Run: node research/fixture-server.mjs 4173
import http from 'node:http';

const port = Number(process.argv[2] ?? 4173);
const filler = 'Coqui is a small tree frog native to Puerto Rico. '.repeat(40);

const pages = {
  '/': `<!doctype html><html><head><title>Coqui demo site</title></head>
    <body><h1>Coqui demo site</h1><h1>Second heading on purpose</h1>
    <p>Short intro text for the fixture.</p>
    <a href="/about">About</a>
    <a href="/products">Products</a>
    <a href="/old-page">Old page</a>
    <a href="/missing">Missing page</a>
    <a href="/blocked">Blocked page</a>
    <a href="https://example.com/external">External</a>
    </body></html>`,
  '/about': `<!doctype html><html><head><title>About Coqui</title>
    <meta name="description" content="About the Coqui crawler.">
    <link rel="canonical" href="/about">
    </head><body><h1>About</h1><p>${filler}</p>
    <a href="/">Home</a><a href="/products">Products</a></body></html>`,
  '/products': `<!doctype html><html><head><title>Products and a deliberately long page title that exceeds sixty characters</title></head>
    <body><h1>Products</h1><p>Thin product copy.</p>
    <a href="/about">About</a><a href="/missing">Broken link</a></body></html>`,
  '/blocked': `<!doctype html><html><head><title>Blocked</title></head><body><h1>Blocked</h1></body></html>`,
};

const server = http.createServer((request, response) => {
  const url = new URL(request.url ?? '/', `http://localhost:${port}`);
  if (url.pathname === '/robots.txt') {
    response.writeHead(200, { 'content-type': 'text/plain' });
    response.end('User-agent: *\nDisallow: /blocked\n');
    return;
  }
  if (url.pathname === '/old-page') {
    response.writeHead(301, { location: '/about' });
    response.end();
    return;
  }
  if (url.pathname === '/missing') {
    response.writeHead(404, { 'content-type': 'text/html' });
    response.end('<!doctype html><html><head><title>Missing</title></head><body><h1>Missing</h1></body></html>');
    return;
  }
  const body = pages[url.pathname];
  if (!body) {
    response.writeHead(404, { 'content-type': 'text/html' });
    response.end('<!doctype html><html><body>not found</body></html>');
    return;
  }
  response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  response.end(body);
});

server.listen(port, '127.0.0.1', () => {
  console.log(`fixture site on http://127.0.0.1:${port}/`);
});
