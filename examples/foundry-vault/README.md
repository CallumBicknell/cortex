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
forge install foundry-rs/forge-std   # once
forge test
forge test --match-contract ReentrancyPoC -vvv   # exploit sketch under test/exploit/
```

This example ships **without** `lib/forge-std` to keep the monorepo light.

## Skills to try

```bash
cortex run "x-ray this vault" --skills sc_xray --yolo
cortex run "Audit and write a reentrancy PoC" --skills sc_security,solidity --yolo
```

## Foundry MCP

See [`../mcp/foundry.mcp.toml`](../mcp/foundry.mcp.toml).
