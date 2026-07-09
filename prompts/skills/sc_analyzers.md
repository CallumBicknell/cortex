# Static analysis & fuzz tooling (Solidity)

Use **real tools** when available. Never fabricate Slither/forge/aderyn output.
If a tool is missing, say so and continue with manual review.

## Detection

| Marker | Prefer |
|--------|--------|
| `foundry.toml` | `forge build`, `forge test`, fuzz/invariant |
| `slither.config.json` / slither on PATH | `slither .` |
| `aderyn.toml` / aderyn on PATH | `aderyn` |
| `echidna.yaml` / echidna | Echidna campaigns |
| `medusa.json` / medusa | Medusa fuzz |

Check with `command -v` or attempt and handle failure.

## Foundry

```bash
forge build
forge test
forge test --match-test testName -vvvv
forge test --fuzz-runs 10000          # when fuzz tests exist
forge coverage                        # if configured
forge snapshot                        # gas, optional
```

- Prefer adding **reproducing tests** under `test/` for confirmed bugs.
- Invariant tests: document broken invariants in the report.

## Slither

```bash
slither .                             # or: slither contracts/ --filter-paths "lib|test|node_modules"
slither . --print human-summary
# triage: reentrancy, arbitrary-send, controlled-delegatecall, uninitialized-state,
#         unprotected-upgrade, suicidal, unchecked-transfer
```

**Never ignore without comment:** reentrancy, arbitrary delegatecall/selfdestruct,
unprotected init/upgrade, unchecked low-level calls on value paths.

Triage noise: exclude `lib/`, known false positives; cite detector name + file:line.

## Aderyn (if present)

```bash
aderyn .
```

Use as a second static opinion; still require code-level proof for findings.

## Echidna / Medusa (if present)

- Run project config if present; do not invent long campaigns without user ask.
- Capture counterexamples into forge PoCs when possible.

## Mythril (optional)

```bash
myth analyze path/to/Contract.sol
```

Heavy; use only when user asks or for small critical contracts.

## Reporting tool results

For each tool run:

```markdown
### Tool: slither
- **Command:** `slither .`
- **Status:** ran | missing | failed
- **Signal:** bullet list of high-confidence issues (or “no high-signal findings”)
- **Not triaged:** note if full output was truncated
```

Then map tool hits into FINDING/LEAD using the shared findings schema
(`skills/findings_schema`).
