# Installing Coqui

Coqui ships as a native CLI binary, a desktop app, and a container image.

## Docker Compose (one command)

```bash
docker compose up --build
```

The API is then at <http://127.0.0.1:8787> and crawl data persists in the `coqui-data` volume mounted at `/data`. The port is bound to loopback by default; if you expose it externally, run the service with `--token`/`COQUI_API_TOKEN`.

Render-based features (JavaScript rendering, axe, Lighthouse) need a Chromium/Chrome binary inside the container. The default image is CLI-only; mount or extend it with Chrome for those features.

## Release binaries

Download the asset for your platform from the GitHub releases page (for example `coqui-linux-x86_64`) plus `SHA256SUMS-<os>-<arch>.txt`, verify the checksum, then run:

```bash
sha256sum -c SHA256SUMS-linux-x86_64.txt
./coqui crawl url https://example.com --out crawl-out
```

The CLI is a self-contained Rust binary: no JVM or Node.js installation is required for crawling, auditing, reporting, exporting, or the API server.

## Build from source

```bash
cargo install --path crates/coqui-cli
coqui serve --out coqui-out --addr 127.0.0.1:8787
```

## Desktop app

Build with the Tauri toolchain:

```bash
npm ci
npm run build:web
npm run build
```

The desktop build currently uses the system Chrome/Edge for rendering; set `COQUI_BROWSER_PATH` to override discovery.
## Rendering browser

JavaScript rendering, axe and Lighthouse need Chrome/Edge. Coqui discovers an installed browser automatically; otherwise install Chrome for Testing into the user cache:

```bash
coqui browser install
coqui browser install --dry-run   # print the download URL without fetching
# verify a hash you obtained from a trusted mirror
coqui browser install --sha256 <64-hex-digest>
coqui browser path
```

`coqui browser install` downloads the current stable Chrome for Testing for the platform, verifies the SHA-256 when `--sha256` is supplied, extracts it under the OS cache directory (`%LOCALAPPDATA%\coqui\browser`, `~/Library/Caches/coqui/browser` or `~/.cache/coqui/browser`), writes `SHA256SUMS.txt`, and prints the executable path. Set `COQUI_BROWSER_PATH` to that path (or run `coqui browser path` to see what would be used).

The container image does not include a browser; extend the image or set `COQUI_BROWSER_PATH` to a mounted Chrome when you need rendering inside Docker.
## Checking for updates

```bash
coqui update           # human-readable
coqui update --json    # current, latest, update_available, url
```

The check queries the latest GitHub release and compares versions; it never installs anything. Desktop updates follow the same release assets.


