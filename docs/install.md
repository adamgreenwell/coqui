# Installing Coqui

Coqui ships as a native CLI binary, a desktop app, and a container image.

## Docker Compose (one command)

```bash
docker compose up --build
```

The API is then at <http://127.0.0.1:8787> and crawl data persists in the `coqui-data` volume mounted at `/data`.

Render-based features (JavaScript rendering, axe, Lighthouse) need a Chromium/Chrome binary inside the container. The default image is CLI-only; mount or extend it with Chrome for those features.

## Release binaries

Download the asset for your platform from the GitHub releases page (`coqui` / `coqui.exe`) plus `SHA256SUMS.txt`, verify the checksum, then run:

```bash
sha256sum -c SHA256SUMS.txt
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
cargo tauri build
```

The desktop build currently uses the system Chrome/Edge for rendering; set `COQUI_BROWSER_PATH` to override discovery.

