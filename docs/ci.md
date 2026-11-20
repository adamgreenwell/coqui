# CI operation

Recipes for running Coqui in CI without the desktop shell.

## Crawl and upload artifacts

```yaml
name: coqui-crawl
on:
  workflow_dispatch:
  schedule:
    - cron: "0 6 * * 1"

jobs:
  crawl:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release -p coqui-cli
      - run: ./target/release/coqui crawl url https://example.com --max-urls 200 --out crawl
      - run: ./target/release/coqui export --out crawl --family all
      - run: ./target/release/coqui report summary --out crawl
      - uses: actions/upload-artifact@v4
        with:
          name: coqui-crawl
          path: crawl/
```

The crawl directory contains `coqui.db` plus CSV/report outputs. Add `--webhook URL` to notify another system on completion.

## Fail the job on issues

Export issues and assert with `jq`:

```bash
./target/release/coqui export --out crawl --family issues
test "$(tail -n +2 crawl/issues.csv | wc -l)" -eq 0
```

Use `coqui issue-settings export/import` to carry suppressions and severity overrides between runs.

## API smoke test (container)

```yaml
jobs:
  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: docker build -t coqui-api .
      - run: |
          mkdir data
          docker run -d --name coqui -p 8787:8787 -v "$PWD/data:/data" -e COQUI_API_TOKEN=secret coqui-api
          for i in $(seq 1 30); do curl -sf http://127.0.0.1:8787/health && break || sleep 1; done
          curl -sf -H "Authorization: Bearer secret" http://127.0.0.1:8787/crawls
          curl -sf -X POST -H "Authorization: Bearer secret" -H "content-type: application/json" \
            -d '{"seed":"https://example.com","maxUrls":5}' http://127.0.0.1:8787/crawls
```

Read endpoints: `/crawls`, `/crawls/{id}`, `/issues`, `/issues/{rule}`, `/urls` (filters), `/export/{family}`, `/report/{kind}`. Start a crawl with `POST /crawls` and poll `GET /crawls/{id}` until `completed`.

## Scheduled crawls in CI

Prefer the workflow `schedule` trigger over in-process scheduling unless you need sub-hour intervals. `coqui schedule` remains useful on a long-lived runner.

## Notes

- Crawls are sequential per process; increase `--concurrency` for faster runs.
- The database is SQLite; cache or artifact the `coqui-out` directory between runs if you need history.
- Use `COQUI_API_TOKEN`/`--token` whenever the API is reachable beyond loopback.
