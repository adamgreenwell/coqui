// Local demo site for Coqui verification.
// Run: npm run fixture  (or node tools/fixture/site.mjs 4173)
import http from 'node:http';

const port = Number(process.argv[2] ?? 4173);
const filler = 'Coqui is a small tree frog native to Puerto Rico. '.repeat(40);
const expectedAuth = `Basic ${Buffer.from('user:pass').toString('base64')}`;

function page(title, body, head = '') {
  return `<!doctype html><html><head><title>${title}</title>${head}</head><body>${body}</body></html>`;
}

const server = http.createServer((request, response) => {
  const url = new URL(request.url ?? '/', `http://localhost:${port}`);
  const origin = `http://${request.headers.host ?? `127.0.0.1:${port}`}`;
  const send = (status, type, body, headers = {}) => {
    response.writeHead(status, { 'content-type': type, ...headers });
    response.end(body);
  };

  switch (url.pathname) {
    case '/robots.txt':
      return send(200, 'text/plain', 'User-agent: *\nDisallow: /blocked\n');
    case '/sitemap.xml':
      return send(
        200,
        'application/xml',
        `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>${origin}/about</loc></url>
  <url><loc>${origin}/products</loc></url>
  <url><loc>${origin}/missing</loc></url>
</urlset>`,
      );
    case '/sitemap-index.xml':
      return send(
        200,
        'application/xml',
        `<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap><loc>${origin}/sitemap.xml</loc></sitemap>
</sitemapindex>`,
      );
    case '/auth':
      if (request.headers.authorization === expectedAuth) {
        return send(200, 'text/html; charset=utf-8', page('Private', '<h1>Private</h1>'));
      }
      return send(401, 'text/html', page('Auth required', '<h1>Auth required</h1>'), {
        'www-authenticate': 'Basic realm="fixture"',
      });
    case '/old-page':
      response.writeHead(301, { location: '/about' });
      return response.end();
    case '/missing':
      return send(404, 'text/html', page('Missing', '<h1>Missing</h1>'));
    case '/blocked':
      return send(200, 'text/html; charset=utf-8', page('Blocked', '<h1>Blocked</h1>'));
    case '/about':
      return send(
        200,
        'text/html; charset=utf-8',
        page(
          'About Coqui',
          `<h1>About</h1><p>${filler}</p><a href="/">Home</a><a href="/products">Products</a>`,
          '<meta name="description" content="About the Coqui crawler."><link rel="canonical" href="/about">',
        ),
      );
    case '/products':
      return send(
        200,
        'text/html; charset=utf-8',
        page(
          'Products and a deliberately long page title that exceeds sixty characters',
          '<h1>Products</h1><p>Thin product copy.</p><a href="/about">About</a><a href="/missing">Broken link</a>',
        ),
      );
    case '/':
      return send(
        200,
        'text/html; charset=utf-8',
        page(
          'Coqui demo site',
          `<h1>Coqui demo site</h1><h1>Second heading on purpose</h1>
           <p>Short intro text for the fixture.</p>
           <a href="/about">About</a>
           <a href="/products">Products</a>
           <a href="/old-page">Old page</a>
           <a href="/missing">Missing page</a>
           <a href="/blocked">Blocked page</a>
           <a href="https://example.com/external">External</a>`,
        ),
      );
    default:
      return send(404, 'text/html', '<html><body>not found</body></html>');
  }
});

server.listen(port, '127.0.0.1', () => {
  console.log(`fixture site on http://127.0.0.1:${port}/`);
});
