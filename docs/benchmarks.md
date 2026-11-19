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

Node v26.13.1, debug build, concurrency 8, no rendering.

## Notes

- Small runs are dominated by startup/DB creation; use ≥2,000 URLs for throughput.
- Rendering, Lighthouse and axe add their own costs and are measured separately.
- The fixture is loopback-only, so network latency is not represented.
- Port must be free; the harness fails fast if the fixture cannot start.
