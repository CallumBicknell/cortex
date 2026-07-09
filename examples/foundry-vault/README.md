# VulnerableVault (demo)

Tiny Foundry-style layout for Cortex multi-lens audit demos.

`VulnerableVault.sol` has a deliberate reentrancy bug in `withdraw()`.

## Use with Cortex

```bash
# From repo root
cortex run "Audit examples/foundry-vault for reentrancy" \
  --workspace . --skills sc_security,solidity --yolo

# Multi-lens tool (agent should call audit_lenses)
cortex skills select "audit this vault with multi-lens"
```

## Optional Foundry

If `forge` is installed and `forge-std` is present under `lib/`:

```bash
cd examples/foundry-vault
forge test
```

This example ships **without** `lib/forge-std` to keep the monorepo light.
Install Foundry std with `forge install foundry-rs/forge-std` if you want live tests.

## Foundry MCP

See [`../mcp/foundry.mcp.toml`](../mcp/foundry.mcp.toml).
