#!/usr/bin/env bash
# Optional Foundry / Web3 smoke. Skips cleanly when tools are missing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== cortex Foundry/Web3 smoke =="

if ! command -v cargo >/dev/null 2>&1; then
  echo "skip: cargo not found"
  exit 0
fi

echo "-- skills select (sc_security)"
cargo run -q -p cortex-cli -- skills select "audit this vault for reentrancy" | tee /tmp/cortex-skills-select.txt
grep -q sc_security /tmp/cortex-skills-select.txt

echo "-- parse outline VulnerableVault.sol"
cargo run -q -p cortex-cli -- parse outline examples/foundry-vault/src/VulnerableVault.sol | tee /tmp/cortex-sol-outline.txt
grep -Eiq 'withdraw|VulnerableVault|contract' /tmp/cortex-sol-outline.txt

if command -v forge >/dev/null 2>&1; then
  echo "-- forge present: $(forge --version | head -1)"
  if [[ -d examples/foundry-vault/lib/forge-std ]]; then
    (cd examples/foundry-vault && forge test -q) || echo "warn: forge test failed (optional)"
  else
    echo "skip: forge-std not installed under examples/foundry-vault/lib"
  fi
else
  echo "skip: forge not on PATH"
fi

if command -v npx >/dev/null 2>&1; then
  MCP_CFG="$ROOT/examples/mcp/foundry.mcp.toml"
  if [[ -f "$MCP_CFG" ]]; then
    echo "-- MCP config present: examples/mcp/foundry.mcp.toml"
    echo "   (enable by copying to .cortex/mcp.toml; live MCP start not forced here)"
  fi
else
  echo "skip: npx not on PATH (Foundry MCP optional)"
fi

echo "-- offline mock run (default mock provider from models.toml if present)"
# Prefer configured offline alias; ignore failures so smoke stays skip-friendly.
cargo run -q -p cortex-cli -- run "Summarize reentrancy risk in examples/foundry-vault" \
  --skills sc_security,solidity --yolo --max-turns 2 >/tmp/cortex-foundry-run.txt 2>&1 \
  || echo "warn: offline run skipped or failed (provider/config)"
echo "smoke notes in /tmp/cortex-foundry-run.txt"

echo "OK: foundry/web3 smoke finished"
