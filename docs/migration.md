# Migrating from Screaming Frog

Coqui imports Screaming Frog's portable crawl files (`.dbseospider`), keeps them as local crawl history, and can push them to a Coqui project so they can be diffed against future crawls.

This is the "bring five years of files" workflow. It is deliberately lossless about structure and deliberately silent about Screaming Frog's issue rules: Coqui imports URLs, status codes, titles, meta descriptions, headings, depth, response times, sizes and links. It does **not** translate Screaming Frog's 344 issue rules into Coqui's rules — an imported run has no issues, and the platform labels issue deltas on imported comparisons as **not measured** rather than inventing results.

## What you need

- A Coqui build with the import and sync commands (`coqui 0.1.0` or later).
- The `.dbseospider` files. Screaming Frog's UI exports them as a single portable file per crawl.
- A Coqui project UUID.
- A workspace token with `crawls:sync` (and `projects:read` if you want the project picker). Create it in the platform under **Workspace → Desktop Sync Tokens**.

## Desktop flow

1. **File → Import → Screaming Frog database…**
2. Choose a `.dbseospider`. The import runs locally and reports the crawl id, seed, URL count and link count.
3. Select **Send to cloud** on the import result (or on the imported row in Recent crawls).
4. Enter the API URL (default `https://api.coquiseo.com`), paste the workspace token, load the project list and pick the project.
5. Send. The imported run appears in the project's crawl history with an **Imported** badge.

The token is kept in memory for the dialog only; it is not written to disk or local storage.

## CLI flow

Import one file:

```bash
coqui import sf --file ~/ScreamingFrog/acme-2021.dbseospider --out ~/coqui-history
```

The command prints the stable crawl id (a UUID v5 of the file's SHA-256). Send it:

```bash
coqui sync <crawlId> \
  --project <project-uuid> \
  --token "$COQUI_SYNC_TOKEN" \
  --out ~/coqui-history
```

`--token` can be omitted when `COQUI_SYNC_TOKEN` (or `COQUI_API_TOKEN`) is set. Use `--api-url` for a staging or local platform:

```bash
coqui sync <crawlId> --project <uuid> --token "$TOKEN" \
  --api-url http://127.0.0.1:8000 --out ~/coqui-history
```

Preview without sending anything:

```bash
coqui sync <crawlId> --project <uuid> --token "$TOKEN" --dry-run
```

`--dry-run` prints the URL/link/issue counts and the endpoints it would call; it performs no HTTP requests.

### Batch: five years of files

Use `--json` for both commands so the loop can read the crawl id. `jq` is convenient but any JSON reader works.

```bash
#!/usr/bin/env bash
set -euo pipefail

COQUI_HISTORY="$HOME/coqui-history"
PROJECT="<project-uuid>"
export COQUI_SYNC_TOKEN="<workspace-token>"

mkdir -p "$COQUI_HISTORY"

find "$HOME/ScreamingFrog" -name '*.dbseospider' -print0 |
  sort -z |
  while IFS= read -r -d '' file; do
    echo "== $file"
    imported=$(coqui import sf --file "$file" --out "$COQUI_HISTORY" --json)
    crawl_id=$(printf '%s' "$imported" | jq -r '.crawlId')
    coqui sync "$crawl_id" --project "$PROJECT" --out "$COQUI_HISTORY" --json \
      | jq -c '{externalCrawlId, source, urls, links, chunks}'
  done
```

The loop is safe to re-run:

- Importing the same file again returns the same crawl id with `"reused": true`; no duplicate local crawl is created.
- Sync is an upsert keyed by that stable id, so re-sending the same import updates one cloud run rather than creating a second one.
- A changed file produces a new hash, so it becomes a new run. That is the desired history.

For very large archives, run in batches (for example one year at a time), keep `--out` on a disk with room for the SQLite files, and avoid parallel imports into the same directory. The importer commits each crawl atomically; a failed import is removed rather than left half-written.

## What the cloud receives

The sync uses the existing desktop contract (`coqui-platform/docs/desktop-sync-contract.md`):

- `POST /api/v1/projects/{project}/crawls/sync` with `source: "import"` and a stable `id`.
- `POST .../crawls/{id}/chunks` with at most 1,000 URLs, 2,500 links and 2,500 issues per request. Imported runs send URLs and links; the issue array is empty.
- `POST .../crawls/{id}/finalize`.

The crawl config is sanitised before it leaves the machine: auth user/password, cookies, proxy URLs, webhook URLs and `Authorization`/`Cookie`/`X-Api-Key` headers are removed. Nothing is persisted to the project except the crawl data.

## What is not imported

- Screaming Frog issue rules and their severities.
- Screaming Frog reports, dashboards, segments and custom configuration.
- JavaScript rendering, axe, Web Vitals, console logs or screenshots (an import is structural).
- Search Console, Analytics, backlink or PageSpeed data.

URLs, status codes, titles, meta descriptions, H1s, depth, response time, size and links are imported. Canonical and robots directives are not currently read from the Screaming Frog database.

## Comparing an imported baseline

In the project, open **Crawl Health → Diff** and pick the imported run as the baseline and a fresh crawl as the target. URL/status/metadata/redirect differences are computed normally. Issue counts are marked **Not measured** when either side is imported, because the imported side has no audit data; the platform does not present them as zero.

Shared diff links work the same way, including for imported baselines.

## Expected results and troubleshooting

- **"no results_*/sql member"** — the file is not a Screaming Frog portable crawl, or it is truncated. Re-export it.
- **No URLs imported** — the database has no internal, non-resource rows; check that the crawl actually completed in Screaming Frog.
- **401 / 403 from the platform** — the token is missing, expired, or lacks `crawls:sync`. Create a new workspace token.
- **409 conflict** — that `externalCrawlId` already belongs to another project or to a non-import source. Import files are keyed by their content hash; use the original project or a fresh export.
- **422 from a chunk** — the client sends within the contract's limits. If you see this, capture the response and the crawl id; it indicates a contract mismatch rather than a file-size issue.
- **Import is slow** — parsing is local. A large file takes time proportional to its URL and link count; the progress dialog reports each stage.
- **Token in shell history** — prefer `export COQUI_SYNC_TOKEN=...` for batches, and rotate the token after a migration session.

## Verification

On a reference fixture derived from the committed parity CSVs:

```bash
coqui import sf --file tools/parity/fixtures/reference/reference.dbseospider \
  --out /tmp/coqui-migration --json
coqui sync <crawlId> --project <uuid> --token "$COQUI_SYNC_TOKEN" \
  --api-url http://127.0.0.1:8000 --out /tmp/coqui-migration --dry-run
```

A real 267-URL Screaming Frog export can be substituted for the fixture; the fixture generator is `tools/parity/make-sf-fixture.mjs`.
