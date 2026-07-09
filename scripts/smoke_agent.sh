#!/usr/bin/env bash
# Smoke test: mock provider one-shot run (no network).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== cargo build -p cortex-cli =="
cargo build -p cortex-cli --quiet

echo "== cortex tools list =="
# Avoid `tools list | head` — Rust panics on SIGPIPE/Broken pipe (os error 32).
TOOLS_OUT="$(mktemp)"
cargo run -q -p cortex-cli -- tools list >"$TOOLS_OUT"
head -20 "$TOOLS_OUT"
rm -f "$TOOLS_OUT"

echo "== cortex models list =="
cargo run -q -p cortex-cli -- models list

echo "== cortex run (mock) =="
cargo run -q -p cortex-cli -- run "smoke test prompt" --json --yolo --max-turns 4

echo "OK: smoke_agent passed"
