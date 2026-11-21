# Benchmarks

`coqui bench` writes a synthetic crawl to a throwaway database and reports storage/query performance. It refuses to overwrite an existing `coqui.db` unless `--force` is passed.

```bash
coqui bench --urls 100000 --out bench-out --json
```

Output fields: `urls`, `insert_ms`, `urls_per_second`, `query_ms` (`all_urls`), `db_bytes` (database + WAL).

## Budgets (local development machine)

| Metric | Budget |
| --- | --- |
| Insert throughput | ≥ 50,000 URLs/s (batch transaction) |
| Query `all_urls` | ≤ 1s per 100k URLs |
| DB size | ≤ 1 KB per URL |
| 1M-row insert | ≤ 30s |
| Tab page p95 at 1M | ≤ 500ms |

## Current measurements (Windows, debug build)

- 10,000 URLs: ~0.10s insert (~100k/s), 39ms query, 3.2 MB DB.
- 100,000 URLs (chunked, 10k per transaction): ~1.03s insert (~97k/s), 400ms query, ~39 MB DB (~400 bytes/URL).
- 1,000,000 URLs (chunked, 10k per transaction): **12.70s insert (~79k/s)**, 5.4s full `all_urls` load, **page p95 90ms**, **337 MB DB (~338 bytes/URL)**.
- 100,000 URLs: page p95 8ms.

Batch writes go through `Store::insert_urls_batch`; `coqui bench` commits in chunks (`--chunk`, default 10,000) so memory stays bounded. Row-by-row `insert_url` remains for crawler paths and measures ~1.4k/s.
## Nightly CI benchmark

`.github/workflows/bench.yml` runs daily at 03:00 UTC (and on demand):

1. builds the CLI;
2. runs `coqui bench --urls 100000 --json`;
3. validates the result with `tools/bench/check.mjs` against `tools/bench/budgets.json` (fails the job on a breach);
4. writes `bench-summary.md` (previous vs current table) and uploads both files as artifacts for trend tracking.
## Run-over-run regression

The nightly job restores the previous `bench.json` from the Actions cache and runs `tools/bench/compare.mjs` with `--max-regression 0.10`:

- throughput drop > 10% fails;
- query latency or bytes/URL increase > 10% fails;
- first run or mismatched URL counts skip the comparison.

The current result is saved back to the cache for the next run.



