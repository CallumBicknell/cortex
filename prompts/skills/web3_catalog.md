# Web3 skills catalog (skills.eth.sh)

Help the user **discover and wire** Web3 agent resources. Do **not** silently
download or install packs — only fetch when the user explicitly asks to import
or inspect a URL.

## Canonical sources

| Resource | URL |
|----------|-----|
| UI catalog | https://skills.eth.sh/ |
| Agent dump | https://skills.eth.sh/llms.txt |
| Cortex recipes | See docs/web3-recipes.md in this repo |

## How to browse

1. Prefer `http_request` GET on `https://skills.eth.sh/llms.txt` when the user
   wants an up-to-date list (requires network approval).
2. Summarize matches by area: **Security**, **Developer**, **DeFi**, **Data**.
3. Distinguish **Skill** (prompt/pack) vs **MCP** (tool server) vs **CLI**.

## Security-focused picks

| Name | Type | Install / URL |
|------|------|----------------|
| Pashov skills | Skill | https://github.com/pashov/skills |
| QuillShield | Skill/Plugin | https://github.com/quillai-network/quillshield_skills |
| ETHSkills | Skill | https://ethskills.com/SKILL.md |
| Foundry MCP | MCP (stdio) | `npx -y @pranesh.asp/foundry-mcp-server` |
| Blockscout MCP | MCP (HTTP) | https://mcp.blockscout.com/mcp |
| Tenderly MCP | MCP (HTTP) | https://mcp.tenderly.co/mcp |

Cortex builtins already cover a strong baseline: `sc_security`, `sc_xray`,
`audit_lenses`, `solidity`, `write_audit_report`.

## Import into Cortex (explicit only)

When the user wants a **SKILL.md** as a Cortex learned skill:

```bash
cortex skills import https://example.com/path/SKILL.md
# or local path:
cortex skills import ./path/to/SKILL.md --id my_pack
```

That writes `.cortex/skills/<id>.toml` + `.cortex/prompts/skills/<id>.md`.
Then: `cortex run "…" --skills <id>`.

For **MCP**, edit `.cortex/mcp.toml` (see `config/mcp.toml` examples) — do not
confuse MCP servers with skill markdown.

## Rules

- No auto-install of third-party plugins into the monorepo.
- Never commit API keys from MCP header templates.
- Prefer Cortex builtins for audits unless the user needs a specific external pack.
