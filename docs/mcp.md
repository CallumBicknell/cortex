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

| Value | Meaning |
|-------|---------|
| `stdio` (default) | Spawn local process; Content-Length JSON-RPC framing |
| `http` / `streamable_http` | Streamable HTTP (MCP 2025-03-26): POST JSON, JSON or SSE response |
| `sse` | Same client path as `http`; if initialize fails, try **legacy** HTTP+SSE `endpoint` event discovery |

### Streamable HTTP example

```toml
[[servers]]
name = "blockscout"
enabled = true
transport = "http"
url = "https://mcp.blockscout.com/mcp"
tool_prefix = "mcp_blockscout"
timeout_secs = 60
# headers = { Authorization = "Bearer $API_TOKEN" }
```

### Security

- Local hosts (`localhost`, `127.0.0.1`, …) are **refused** unless `CORTEX_MCP_ALLOW_LOCAL=1`.
- Header values expand `$VAR` / `${VAR}` from the environment (do not commit secrets).
- MCP tools inherit Cortex permission modes (`ask` by default unless listed in `security.toml`). Prefer explicit allow-lists for untrusted servers.

## CLI

MCP tools are loaded automatically during bootstrap when config is present. Use:

```bash
cortex tools list
```

to see both builtins and MCP-prefixed tools.

## Web3 servers (skills.eth.sh)

Commented templates live in `config/mcp.toml` for Foundry MCP (stdio), Blockscout,
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
builtin `sc_security` skill (and `audit_lenses`) is enough to start; remote MCP
adds onchain reads and simulation. See [`docs/web3-security.md`](web3-security.md).

## Browser

Prefer attaching to a CDP endpoint (Obscura, Chrome) via native browser tools — see [`docs/browser.md`](browser.md). Alternatively, enable a Playwright/Puppeteer MCP server entry for richer automation.
