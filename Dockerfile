# Build stage
FROM rust:1.90-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p coqui-cli

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/coqui /usr/local/bin/coqui
WORKDIR /data
EXPOSE 8787
ENTRYPOINT ["coqui", "serve", "--addr", "0.0.0.0:8787", "--out", "/data"]

