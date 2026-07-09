# Coding agent + smart contract security

Cortex is a **coding agent loop** with first-class **Solidity / EVM security**
capability packs. Skills are not hard-coded modes: they activate tools + prompts
from the user message and project fingerprint (e.g. `foundry.toml`).

## Identity

| Layer | Role |
|-------|------|
| `system` prompt | Coding agent + SC security orientation |
| `coding` skill | Always-on file/edit navigation |
| `solidity` skill | Implement contracts, Foundry/Hardhat workflows |
| `sc_security` skill | Audits, vuln finding, analyzers + PoC guidance |
| `sc_xray` skill | Pre-audit readiness / threat map report |
| `audit_lenses` tool | Parallel specialty sub-agents (Cortex-native) |
| Runtime `security` prompt | Agent OS constraints (secrets, sandbox) |

## CLI examples

```bash
# Web3 scaffold (MCP + instructions + foundry_helpers plugin)
cortex init --web3
cortex tools list | grep forge_

# Implement / fix contracts (auto-selects solidity on Foundry projects)
cortex run "Add a withdraw function with CEI and a forge test"

# Explicit security pack
cortex run "Audit contracts/ for reentrancy and oracle risk" --skills sc_security,solidity

# Demo fixture (intentional reentrancy)
cortex run "Audit examples/foundry-vault with multi-lens" --skills sc_security --yolo

# Preview selection
cortex skills select "find vulns in this vault"
cortex skills list

# Solidity outlines
cortex parse outline examples/foundry-vault/src/VulnerableVault.sol
```

### Foundry helpers plugin

`plugins/foundry_helpers` (also copied by `init --web3` into `.cortex/plugins/`) registers fixed-arg tools: `forge_version`, `forge_build`, `forge_test`, `forge_test_verbose`, `forge_test_match`, `forge_test_fuzz`, `forge_fmt_check`. Prefer these over freeform `shell` when auditing. Missing `forge` fails honestly.

## Audit loop

Typical flow (`prompts/skills/sc_security.md`):

1. Map sources and entry points (`code_outline`, glob)
2. Prefer **`audit_lenses`** for parallel specialty passes:
   - `access` — auth / roles / signatures
   - `reentrancy` — CEI / external calls
   - `economic` — oracles, tokens, 4626, MEV
   - `proxy` — upgrades / storage / delegatecall
   - `invariants` — optional first-principles lens
3. Tool builds `.cortex/tmp/audit-*/source.md` (excludes lib/test/mocks)
4. Orchestrator **dedupes** FINDING/LEAD by (contract, function, bug_class)
5. Optionally `forge build` / `forge test` / `slither .` when installed
6. Structured final report with severity counts

Specialty prompts live under `prompts/skills/audit_lenses/`.

### Tooling depth (P21)

| Pack | Role |
|------|------|
| `sc_xray` | Scope, entry points, trust model, test gaps, risk surfaces → `x-ray.md` |
| `sc_analyzers` | Honest `forge` / Slither / Aderyn / fuzz conventions |
| `sc_poc` | Foundry exploit tests under `test/exploit/` |
| `findings_schema` | Shared FINDING/LEAD markdown + JSON shape |

Demo PoC sketch: `examples/foundry-vault/test/exploit/ReentrancyPoC.t.sol`.

### Audit artifacts (P22)

End of audit / x-ray → tool **`write_audit_report`**:

- Path: `.cortex/audits/<ts>-<title>-{audit|xray}-report.md`
- Optional: sibling `.json` findings + `.meta.json` (session id, fingerprint)
- Eval fixtures: `evals/fixtures/vulns/` + `evals/sc_*.toml`

```bash
cortex eval run   # includes sc_reentrancy_finding, sc_xray_shape, …
```

This is **not** a vendored Pashov 12-agent tree; deeper external packs remain at
[skills.eth.sh](https://skills.eth.sh/) (Pashov, QuillShield).

### Skill import bridge (P23)

```bash
cortex skills import ./path/to/SKILL.md --id my_pack
cortex skills import https://example.com/SKILL.md --dry-run   # preview only
cortex run "…" --skills my_pack
```

- Catalog skill: `web3_catalog` (skills.eth.sh guidance; no silent downloads)
- Recipes: [`docs/web3-recipes.md`](web3-recipes.md)

## Project fingerprint

`ProjectInfo` treats as Solidity/EVM signals:

- `foundry.toml`, Hardhat/Truffle configs
- `remappings.txt`
- `slither.config.json` (optional)
- test command `forge test` when Foundry is present

## First Foundry session

1. Install Foundry: https://getfoundry.sh
2. Optional MCP (stdio):

```bash
cp examples/mcp/foundry.mcp.toml .cortex/mcp.toml
# requires Node + npx; tools appear as mcp_foundry_*
cortex tools list
```

3. Point Cortex at a Foundry project (or the demo vault):

```bash
cortex run "Audit examples/foundry-vault for reentrancy" \
  --skills sc_security,solidity --yolo
```

4. Optional smoke (skips missing tools):

```bash
./scripts/smoke_foundry.sh
```

Demo layout: `examples/foundry-vault/` (vulnerable `withdraw`).

## Web3 skills catalog (skills.eth.sh)

Cortex does **not** vendor third-party skill repos. Discover and wire them via:

- UI: [https://skills.eth.sh/](https://skills.eth.sh/)
- Agent dump: [https://skills.eth.sh/llms.txt](https://skills.eth.sh/llms.txt)

### Security-focused

| Resource | What it adds |
|----------|----------------|
| [Pashov skills](https://github.com/pashov/skills) | solidity-auditor, x-ray pre-audit, fizz (Echidna/Medusa) |
| [QuillShield skills](https://github.com/quillai-network/quillshield_skills) | Multi-plugin audit methodology |
| [ETHSkills security](https://ethskills.com/) | Defensive patterns (basis for builtin checklist) |

### MCP (configure in `.cortex/mcp.toml`)

| Server | Install / URL |
|--------|----------------|
| Foundry MCP | `examples/mcp/foundry.mcp.toml` / `npx -y @pranesh.asp/foundry-mcp-server` |
| Blockscout | `https://mcp.blockscout.com/mcp` (SSE; transport TBD) |
| Tenderly | `https://mcp.tenderly.co/mcp` (SSE; transport TBD) |
| CoinGecko | `https://mcp.api.coingecko.com/mcp` |
| Cryo | `uvx cryo-mcp --rpc-url $ETH_RPC_URL` |

stdio **and** Streamable HTTP (`transport = "http"`) are supported. Legacy SSE
endpoint discovery is used as a fallback when initialize fails. See
[`docs/mcp.md`](mcp.md).

## Honest limits

- Builtin `sc_security` / `audit_lenses` is **agent-assisted review**, not a
  substitute for a professional audit or formal verification.
- Findings without proof should stay **LEADs**.
- Tool output must be real — if Slither/forge is missing, the agent should say so.

## Related docs

- [`docs/skills.md`](skills.md) — skill selection model
- [`docs/mcp.md`](mcp.md) — MCP loading
- [`docs/parse.md`](parse.md) — Solidity outlines
- [`docs/security.md`](security.md) — agent OS policy / sandbox
- [`prompts/skills/sc_security.md`](../prompts/skills/sc_security.md)
- [`prompts/skills/solidity.md`](../prompts/skills/solidity.md)
