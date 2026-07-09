# Justfile — local CI parity (https://github.com/casey/just)
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default: build test

help:
    @echo "just targets: build release install install-debug test lint fmt clippy eval smoke deny python-test ci docker clean"

build:
    cargo build --workspace

release:
    cargo build --release -p cortex-cli

# Install current tree → ~/.local/bin/cortex
install:
    ./scripts/install-local.sh

install-debug:
    CORTEX_BUILD_PROFILE=debug ./scripts/install-local.sh

test:
    cargo test --workspace --all-targets

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

eval:
    cargo build -p cortex-cli --quiet
    cargo run -q -p cortex-cli -- eval run --dir evals

smoke:
    ./scripts/smoke_agent.sh

deny:
    cargo deny check

python-test:
    cd sdks/python && (test -d .venv || python3 -m venv .venv) && .venv/bin/pip install -q -e ".[dev]" && .venv/bin/pytest -q

ci: lint test eval smoke python-test
    @echo "OK: local CI suite passed"

docker:
    docker build -t cortex:local .

clean:
    cargo clean
