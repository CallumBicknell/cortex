#!/usr/bin/env bash
# Run the same gates as GitHub Actions CI (minus multi-OS release builds).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== fmt =="
cargo fmt --all -- --check

echo "== clippy =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== test =="
cargo test --workspace --all-targets

echo "== eval =="
cargo build -p cortex-cli --quiet
cargo run -q -p cortex-cli -- eval run --dir evals

echo "== smoke =="
./scripts/smoke_agent.sh

if command -v cargo-deny >/dev/null 2>&1; then
  echo "== cargo-deny =="
  cargo deny check
else
  echo "== cargo-deny (skipped; install: cargo install cargo-deny) =="
fi

echo "== python sdk =="
cd sdks/python
if [[ ! -d .venv ]]; then
  python3 -m venv .venv
fi
.venv/bin/pip install -q -e ".[dev]"
.venv/bin/pytest -q
cd "$ROOT"

echo "OK: scripts/ci_local.sh passed"
