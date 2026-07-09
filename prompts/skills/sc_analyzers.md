# Static analysis & fuzz tooling (Solidity)

Use **real tools** when available. Never fabricate Slither/forge/aderyn output.
If a tool is missing, say so and continue with manual review.

## Prefer fixed plugin tools (when registered)

After `cortex init --web3` (or when monorepo `plugins/` is discovered):

| Tool | Role |
|------|------|
| `forge_build` / `forge_test` / `forge_test_match` / `forge_test_fuzz` | Foundry helpers |
| `slither_version` / `slither_scan` / `slither_scan_path` / `slither_human_summary` | Slither |
| `aderyn_version` / `aderyn_scan` | Aderyn |

Prefer these over freeform `shell`. They fail honestly if the binary is absent.
Slither tools may return findings with a non-zero exit **and** still include full output
(`allow_nonzero` on the plugin tools).

## Detection

| Marker | Prefer |
|--------|--------|
| `foundry.toml` | `forge_build`, `forge_test`, fuzz/invariant |
| `slither.config.json` / slither on PATH | `slither_scan` |
| `aderyn.toml` / aderyn on PATH | `aderyn_scan` |
| `echidna.yaml` / echidna | Echidna campaigns (shell if no plugin) |
| `medusa.json` / medusa | Medusa fuzz (shell if no plugin) |

If fixed tools are not registered, fall back to shell:

```bash
command -v slither && slither .
command -v aderyn && aderyn .
```

## Foundry

Prefer `forge_*` tools. Shell fallback:

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

Prefer `slither_scan` / `slither_human_summary`. Shell fallback:

```bash
slither . --filter-paths "lib|test|node_modules"
slither . --print human-summary
# triage: reentrancy, arbitrary-send, controlled-delegatecall, uninitialized-state,
#         unprotected-upgrade, suicidal, unchecked-transfer
```

**Never ignore without comment:** reentrancy, arbitrary delegatecall/selfdestruct,
unprotected init/upgrade, unchecked low-level calls on value paths.

Triage noise: exclude `lib/`, known false positives; cite detector name + file:line.

## Aderyn (if present)

Prefer `aderyn_scan`. Shell fallback: `aderyn .`

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
### Tool: slither_scan
- **Command:** `slither . --filter-paths …` (or plugin tool name)
- **Status:** ran | missing | failed
- **Signal:** bullet list of high-confidence issues (or “no high-signal findings”)
- **Not triaged:** note if full output was truncated
```

Then map tool hits into FINDING/LEAD using the shared findings schema
(`skills/findings_schema`).
