# Solidity / EVM development

You write and review **Solidity** and Foundry-oriented smart contracts as part of
a normal coding agent loop. Security is not optional when code holds or moves
value — apply `sc_security` habits while implementing, not only at the end.

## Project detection

Prefer tooling based on markers:

| Marker | Prefer |
|--------|--------|
| `foundry.toml` | Foundry: `forge build`, `forge test`, `forge fmt` |
| `hardhat.config.*` | Hardhat: compile / test scripts in package.json |
| `truffle-config.js` | Truffle (legacy) |
| `remappings.txt` / `lib/` | Foundry deps layout |

## Coding standards

- Match the repo’s Solidity version, style, and OpenZeppelin (or solmate) usage.
- Prefer explicit visibility, custom errors (when project uses them), and events
  on state changes.
- Use CEI + reentrancy guards on external value transfers.
- Never hardcode 18 decimals for arbitrary ERC-20s; read `decimals()` or use
  known constants carefully.
- Prefer SafeERC20 for transfers/approvals.
- Avoid unbounded loops over user-growable arrays for critical paths.
- Document trust assumptions (admin keys, oracles, allowed tokens) in NatSpec
  or adjacent docs when introducing them.

## Foundry workflow

```bash
forge build
forge test
forge test --match-test testName -vvvv   # deep traces
forge test --fuzz-runs 10000             # when fuzz tests exist
forge coverage                           # if configured
forge fmt
```

- Put unit/integration tests under `test/` as `*.t.sol`.
- Prefer invariant / fuzz tests for accounting-heavy logic.
- Use `vm.prank`, `vm.expectRevert`, and deal helpers instead of fragile setup.
- For exploit reproduction, add a failing-then-fixed forge test rather than only
  prose.

## Static analysis

When installed, run before claiming readiness:

```bash
slither .
# optional: mythril, aderyn, etc. if present in the project
```

Treat high-confidence Slither findings seriously (reentrancy, arbitrary
delegatecall, unprotected init, unchecked low-level calls).

## When the task is an audit

If the user asks to audit, find vulns, threat-model, or pre-deploy review:

1. Activate full security methodology (same as skill `sc_security`).
2. Produce structured findings with severity and proof.
3. Offer PoC tests for Critical/High when practical.

If they only asked to implement a feature, still:

- Avoid introducing the classic vulns (reentrancy, open auth, spot oracles,
  vault inflation, infinite approvals without reason).
- Call out residual risk when you leave privileged roles or upgrade paths.

## Onchain / Web3 tooling

Discover agent resources at https://skills.eth.sh/ . Useful defaults:

- **Foundry MCP** — forge/cast/anvil from MCP if configured
- **Blockscout MCP** — verified contracts and chain reads
- **Tenderly** — simulation/traces
- **Pashov / QuillShield / ETHSkills** — deeper audit skill packs the user can
  install separately; Cortex’s built-in `sc_security` covers the core loop

## Do not

- Deploy with real keys or mainnet value without explicit user request.
- Log or commit private keys, mnemonics, or RPC secrets.
- Claim production readiness without tests + static analysis caveats.
