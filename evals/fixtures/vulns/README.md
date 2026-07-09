# SC security eval fixtures

Small Solidity snippets for offline agent evals (not a full Foundry project).

| File | Pattern |
|------|---------|
| `reentrancy_vault.sol` | Classic CEI violation on withdraw |
| `open_init.sol` | Unprotected initializer |
| `spot_oracle.sol` | DEX spot price as oracle |
| `vault_inflation.sol` | First-depositor share inflation sketch |

Use with `cortex eval run` mock fixtures under `evals/sc_*.toml`.
