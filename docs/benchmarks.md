# Benchmarks

Repeatable crawl benchmarks against local fixtures. Results are environment-specific; use them to spot regressions on the same machine.

## Running

```bash
npm run bench -- --urls 2000 --port 4195
```

The harness:

1. builds the `coqui` CLI,
2. starts `tools/fixture/bulk.mjs` with N pages,
3. crawls the fixture sitemap with `--max-urls N`,
4. records wall time, URLs/s, crawled/discovered/issues, DB and CSV sizes,
5. writes `bench/<timestamp>/result.json` and `bench/latest.json`.

`bench/` is generated output and not committed.

## Baseline (Windows, local fixture)

| URLs | Time | URLs/s | Issues | DB | CSV |
| --- | --- | --- | --- | --- | --- |
| 50 | 1.763 s | 28.4 | 100 | 148 KB | 7.5 KB |
| 2,000 | 6.833 s | 292.7 | 4,000 | 2.35 MB | 311 KB |
| 10,000 | 35.793 s | 279.4 | 20,000 | 11.3 MB | 1.54 MB |
| 50,000 | 180.294 s | 277.3 | 100,000 | 57.1 MB | 7.91 MB |
| 100 rendered | 64.1 s | 1.6 | 307 | 320 KB | 15 KB |

Streaming CSV export: 50,000 rows in 0.044 s (2.12 MB output) from the 50k crawl database.

Node v26.13.1, debug build, concurrency 8. Rendered runs use `--render --screenshots --a11y --vitals`.

## Notes

- Small runs are dominated by startup/DB creation; use ≥2,000 URLs for throughput.
- Rendering, Lighthouse and axe add their own costs and are measured separately.
- The fixture is loopback-only, so network latency is not represented.
- Port must be free; the harness fails fast if the fixture cannot start.

