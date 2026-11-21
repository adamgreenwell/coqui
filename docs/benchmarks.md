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

## Current measurements (Windows, debug build)

- 10,000 URLs: ~0.10s insert (~100k/s), 39ms query, 3.2 MB DB.
- 100,000 URLs: ~1.08s insert (~92k/s), 400ms query, ~64 MB DB (~660 bytes/URL).

Batch writes go through `Store::insert_urls_batch`, which wraps one SQLite transaction. Row-by-row `insert_url` remains for crawler paths and measures ~1.4k/s.
