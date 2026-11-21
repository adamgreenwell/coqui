# CI operation

Recipes for running Coqui in CI without the desktop shell. The repository's own workflows are in `.github/workflows/`:

| Workflow | Trigger | What it does |
| --- | --- | --- |
| `ci.yml` | push to `main`, every PR | Rust formatting + tests, Next.js static export, Tauri shell check, parity fixture gate, documentation check |
| `bench.yml` | nightly + manual | 100k-URL storage benchmark, budget check, regression comparison, trend artifacts |
| `parity-report.yml` | release + manual | compares committed fixtures and attaches/upload the parity report |
| `release.yml` | `v*` tags | CLI binaries and desktop installers with checksums (see [release.md](release.md)) |

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

Prefer the built-in gate:

```bash
./target/release/coqui crawl url https://example.com --out crawl --fail-on-issues 0
```

It exits 3 when the crawl produces more issues than the threshold. Or export and assert with `jq`/`wc`:

```bash
./target/release/coqui export --out crawl --family issues
test "$(tail -n +2 crawl/issues-*.csv | wc -l)" -eq 0
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

Endpoints: `/crawls`, `/crawls/{id}`, `/crawls/{id}/issues`, `/crawls/{id}/issues/{rule}`, `/crawls/{id}/urls` (filters), `/crawls/{id}/links?url=`, `/crawls/{id}/export/{family}`, `/crawls/{id}/report/{kind}`. Start a crawl with `POST /crawls` and poll `GET /crawls/{id}` until `completed`. See [api.md](api.md).

## Scheduled crawls in CI

Prefer the workflow `schedule` trigger over in-process scheduling unless you need sub-hour intervals. `coqui schedule` remains useful on a long-lived runner; `coqui schedule run-due` must be invoked by cron/Task Scheduler/CI.

## GitLab CI

```yaml
stages: [crawl]

crawl:
  stage: crawl
  image: rust:1.90
  script:
    - cargo build --release -p coqui-cli
    - ./target/release/coqui crawl url "$SEED" --max-urls 200 --json --out crawl | tee crawl.json
    - ./target/release/coqui export --out crawl --family all
  artifacts:
    paths:
      - crawl/
      - crawl.json
```

`--json` gives one machine-readable summary; `--ndjson` streams progress events for log viewers. The same recipes work on a self-hosted runner.

## Multi-arch container

```bash
docker buildx create --use
docker buildx build --platform linux/amd64,linux/arm64 \
  -t ghcr.io/adamgreenwell/coqui-api:latest --push .
```

Buildx needs QEMU for cross-building (`docker run --privileged --rm tonistiigi/binfmt --install all`). The current Dockerfile ships the CLI/API only, without Chromium/Node; rendering and Lighthouse need a browser-enabled image or the host sidecars.

## Notes

- Crawls are sequential per process; increase `--concurrency` for faster runs.
- The database is SQLite; cache or artifact the `coqui-out` directory between runs if you need history.
- Use `COQUI_API_TOKEN`/`--token` whenever the API is reachable beyond loopback.
- The documentation check runs `node tools/docs/check.mjs`; it verifies counts, report kinds, export families and relative links against the code.
