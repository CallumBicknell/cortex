# MCP integration

Cortex can load tools from [Model Context Protocol](https://modelcontextprotocol.io) servers and register them as first-class `Tool` implementations.

## Config

Place servers in `config/mcp.toml` or `.cortex/mcp.toml` (or `CORTEX_MCP_CONFIG`):

```toml
[[servers]]
name = "filesystem"
enabled = true
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/absolute/workspace"]
tool_prefix = "mcp_fs"
```

Tools appear as `mcp_fs_<toolname>` (or your prefix). Disabled servers are ignored so the agent starts offline by default.

## Transport

- **stdio** — supported (Content-Length framing, initialize + tools/list + tools/call)
- **SSE/HTTP** — reserved in config; not implemented yet

## CLI

MCP tools are loaded automatically during bootstrap when config is present. Use:

```bash
cortex tools list
```

to see both builtins and MCP-prefixed tools.

## Browser

Prefer attaching to a CDP endpoint (Obscura, Chrome) via native browser tools — see [`docs/browser.md`](browser.md). Alternatively, enable a Playwright/Puppeteer MCP server entry for richer automation.

## Web3 servers (skills.eth.sh)

Commented templates live in `config/mcp.toml` for Foundry MCP, Blockscout,
Tenderly, CoinGecko, and Cryo. Full catalog: [https://skills.eth.sh/](https://skills.eth.sh/)
and [llms.txt](https://skills.eth.sh/llms.txt).

### First Foundry MCP session

```bash
# Requires Node/npx + Foundry on PATH
cp examples/mcp/foundry.mcp.toml .cortex/mcp.toml
cortex tools list   # look for mcp_foundry_*
```

Demo vulnerable vault: `examples/foundry-vault/`. Skip-friendly smoke:

```bash
./scripts/smoke_foundry.sh
```

For smart-contract audits, local `forge` / `slither` via the shell tool plus the
builtin `sc_security` skill (and `audit_lenses`) is enough to start; MCP adds
onchain reads and simulation. See [`docs/web3-security.md`](web3-security.md).

## Security

MCP tools inherit the same permission modes as other tools (`ask` by default unless listed in `security.toml`). Prefer explicit allow-lists for untrusted servers.
