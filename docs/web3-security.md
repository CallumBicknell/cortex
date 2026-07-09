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
| `sc_security` skill | Audits, vuln finding, threat models, pre-deploy |
| Runtime `security` prompt | Agent OS constraints (secrets, sandbox) |

## CLI examples

```bash
# Implement / fix contracts (auto-selects solidity on Foundry projects)
cortex run "Add a withdraw function with CEI and a forge test"

# Explicit security pack
cortex run "Audit contracts/ for reentrancy and oracle risk" --skills sc_security,solidity

# Preview selection
cortex skills select "find vulns in this vault"
cortex skills list
```

Typical audit loop the agent follows (see `prompts/skills/sc_security.md`):

1. Map sources and entry points
2. `forge build` / `forge test`
3. `slither .` when installed
4. Manual checklist (reentrancy, auth, tokens, oracles, proxies, MEV)
5. Structured findings with severity + proof (forge PoC when useful)
6. Optional parallel specialty passes via `spawn_subagent`

## Project fingerprint

`ProjectInfo` treats as Solidity/EVM signals:

- `foundry.toml`, Hardhat/Truffle configs
- `remappings.txt`
- `slither.config.json` (optional)
- test command `forge test` when Foundry is present

## Web3 skills catalog (skills.eth.sh)

Cortex does **not** vendor third-party skill repos. Discover and wire them via:

- UI: [https://skills.eth.sh/](https://skills.eth.sh/)
- Agent dump: [https://skills.eth.sh/llms.txt](https://skills.eth.sh/llms.txt)

### Security-focused

| Resource | What it adds |
|----------|----------------|
| [Pashov skills](https://github.com/pashov/skills) | solidity-auditor, x-ray pre-audit, fizz (Echidna/Medusa) |
| [QuillShield skills](https://github.com/quillai-network/quillshield_skills) | Multi-plugin audit methodology (invariants, reentrancy, oracles, proxies) |
| [ETHSkills security](https://ethskills.com/) | Defensive patterns + pre-deploy checklist (basis for builtin `sc_security` prompt) |

Install external packs as Claude/Codex skills, or capture workflows into
`.cortex/skills/` with `skill_save` / `skill_creator`.

### MCP (configure in `.cortex/mcp.toml`)

Examples are commented in `config/mcp.toml`:

| Server | Install / URL |
|--------|----------------|
| Foundry MCP | `npx -y @pranesh.asp/foundry-mcp-server` |
| Blockscout | `https://mcp.blockscout.com/mcp` |
| Tenderly | `https://mcp.tenderly.co/mcp` |
| CoinGecko | `https://mcp.api.coingecko.com/mcp` |
| Cryo | `uvx cryo-mcp --rpc-url $ETH_RPC_URL` |

stdio MCP is supported today; SSE/HTTP entries are reserved until transport is
fully implemented — prefer stdio Foundry MCP + local `forge`/`slither` for audits.

## Honest limits

- Builtin `sc_security` is an **agent-assisted review**, not a substitute for a
  professional audit or formal verification.
- Findings without proof should stay leads.
- Tool output must be real — if Slither/forge is missing, the agent should say so.

## Related docs

- [`docs/skills.md`](skills.md) — skill selection model
- [`docs/mcp.md`](mcp.md) — MCP loading
- [`docs/security.md`](security.md) — agent OS policy / sandbox
- [`prompts/skills/sc_security.md`](../prompts/skills/sc_security.md)
- [`prompts/skills/solidity.md`](../prompts/skills/solidity.md)
