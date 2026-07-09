# Web3 recipes (skills.eth.sh → Cortex)

Practical wiring for common packs from [skills.eth.sh](https://skills.eth.sh/).
Cortex does **not** auto-download third-party skills; every import or MCP enable
is an explicit user action.

## Catalog

```bash
# List remote catalog (network)
curl -sL https://skills.eth.sh/llms.txt | head -80

# Or ask the agent
cortex run "What security skills are on skills.eth.sh?" --skills web3_catalog --yolo
```

## Builtin baseline (no install)

| Need | Cortex |
|------|--------|
| Implement Solidity | `--skills solidity` |
| Full audit | `--skills sc_security` + `audit_lenses` |
| Pre-audit map | `--skills sc_xray` |
| Save report | `write_audit_report` tool |

## Import a SKILL.md

```bash
# From URL (requires network)
cortex skills import https://ethskills.com/security --id ethskills_security

# From a cloned repo path
git clone https://github.com/pashov/skills /tmp/pashov-skills
cortex skills import /tmp/pashov-skills/solidity-auditor --id pashov_auditor

cortex skills list
cortex run "Use the imported guidance" --skills pashov_auditor --yolo
```

Import writes:

- `.cortex/skills/<id>.toml` — capability pack (tools/tags/prompt id)
- `.cortex/prompts/skills/<id>.md` — full skill body

## MCP recipes

Copy snippets into `.cortex/mcp.toml` and set `enabled = true`.

### Foundry (stdio)

```toml
[[servers]]
name = "foundry"
enabled = true
command = "npx"
args = ["-y", "@pranesh.asp/foundry-mcp-server"]
tool_prefix = "mcp_foundry"
```

Or: `cp examples/mcp/foundry.mcp.toml .cortex/mcp.toml`

### Blockscout (HTTP)

```toml
[[servers]]
name = "blockscout"
enabled = true
transport = "http"
url = "https://mcp.blockscout.com/mcp"
tool_prefix = "mcp_blockscout"
```

### CoinGecko (HTTP, public)

```toml
[[servers]]
name = "coingecko"
enabled = true
transport = "http"
url = "https://mcp.api.coingecko.com/mcp"
tool_prefix = "mcp_cg"
```

### Tenderly (HTTP + auth)

```toml
[[servers]]
name = "tenderly"
enabled = true
transport = "http"
url = "https://mcp.tenderly.co/mcp"
tool_prefix = "mcp_tenderly"
headers = { Authorization = "Bearer $TENDERLY_ACCESS_KEY" }
```

Then: `cortex tools list | grep mcp_`

## External packs (links only)

| Pack | Notes |
|------|--------|
| [Pashov skills](https://github.com/pashov/skills) | Multi-lens auditor, x-ray, fizz — import SKILL.md or use as reference; Cortex has native `audit_lenses` / `sc_xray` |
| [QuillShield](https://github.com/quillai-network/quillshield_skills) | Claude plugin style; import individual skill markdown if available |
| [ETHSkills](https://ethskills.com/) | Modular Ethereum + security modules |

## Safety

- Review imported prompt bodies before production use.
- Prefer `CORTEX_MCP_ALLOW_LOCAL=1` only for local MCP during development.
- Never commit secrets in `.cortex/mcp.toml`.
