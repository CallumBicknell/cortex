# Multi-stage build for the Cortex CLI + API binary.
# Usage:
#   docker build -t cortex:local .
#   docker run --rm -p 8080:8080 -v "$PWD:/workspace" -w /workspace cortex:local serve --bind 0.0.0.0:8080

# syntax=docker/dockerfile:1.6

FROM rust:1.85-bookworm AS builder
WORKDIR /src

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency build with manifests only.
COPY Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml ./
COPY crates ./crates
COPY migrations ./migrations
COPY prompts ./prompts
COPY config ./config
COPY evals ./evals
COPY plugins ./plugins

# Build release CLI
RUN cargo build --release -p cortex-cli \
    && strip target/release/cortex || true

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 cortex

COPY --from=builder /src/target/release/cortex /usr/local/bin/cortex

USER cortex
WORKDIR /workspace
EXPOSE 8080

ENTRYPOINT ["cortex"]
CMD ["--help"]
