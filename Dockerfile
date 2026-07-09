# Multi-stage Cortex CLI build optimized for CI (BuildKit + cargo-chef).
#
# Usage:
#   docker build -t cortex:local .
#   docker run --rm -p 8080:8080 -v "$PWD:/workspace" -w /workspace \
#     cortex:local serve --bind 0.0.0.0:8080
#
# Requires BuildKit (default on modern Docker / docker/build-push-action).

# syntax=docker/dockerfile:1.7

# ---------------------------------------------------------------------------
# Base: cargo-chef + system deps (cached as one layer)
# ---------------------------------------------------------------------------
FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /src

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CARGO_TERM_COLOR=always \
    CARGO_INCREMENTAL=0

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# ---------------------------------------------------------------------------
# Planner: compute dependency recipe (invalidates only when manifests change)
# ---------------------------------------------------------------------------
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# include_str! paths needed so chef can parse the workspace graph
COPY migrations ./migrations
COPY prompts ./prompts
COPY config ./config
RUN cargo chef prepare --recipe-path recipe.json

# ---------------------------------------------------------------------------
# Builder: cook deps (cache mounts) then compile sources
# ---------------------------------------------------------------------------
FROM chef AS builder

# Dependency layer — reused when only .rs files change
COPY --from=planner /src/recipe.json recipe.json
RUN --mount=type=cache,id=cortex-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cortex-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cortex-target,target=/src/target,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json -p cortex-cli

# Application sources (this layer invalidates on code change)
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
COPY prompts ./prompts
COPY config ./config

# Compile; copy binary out of the cache mount into the image layer
RUN --mount=type=cache,id=cortex-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cortex-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cortex-target,target=/src/target,sharing=locked \
    cargo build --release -p cortex-cli \
    && cp /src/target/release/cortex /src/cortex \
    && strip /src/cortex || true

# ---------------------------------------------------------------------------
# Runtime: minimal image
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 cortex

COPY --from=builder /src/cortex /usr/local/bin/cortex

USER cortex
WORKDIR /workspace
EXPOSE 8080

ENTRYPOINT ["cortex"]
CMD ["--help"]
