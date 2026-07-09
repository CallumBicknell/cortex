# Browser automation (CDP)

Cortex drives headless browsers over the **Chrome DevTools Protocol**.
That means you can use:

| Backend | How to run | Default endpoint |
|---------|------------|------------------|
| **[Obscura](https://github.com/h4ckf0r0day/obscura)** | `obscura serve --port 9222` | `ws://127.0.0.1:9222/devtools/browser` |
| **Chrome / Chromium** | `chromium --remote-debugging-port=9222 --headless=new` | discovered via `http://127.0.0.1:9222/json/version` |
| **Custom** | any CDP-compatible browser | set `cdp_url` |

## Quick start (Obscura)

```bash
# Terminal 1
obscura serve --port 9222
# optional stealth:
# obscura serve --port 9222 --stealth

# Terminal 2
cargo run -p cortex-cli -- run "Open https://example.com and tell me the title" \
  --skills browser --yolo
```

Docker:

```bash
docker run -d --name obscura -p 127.0.0.1:9222:9222 h4ckf0r0day/obscura
```

## Config

`config/browser.toml` or `.cortex/browser.toml` (or `CORTEX_BROWSER_CONFIG`):

```toml
enabled = true
backend = "obscura"   # obscura | chrome | custom
cdp_url = ""          # optional explicit ws://...
discovery_url = ""    # optional http://host:port/json/version
host = "127.0.0.1"
port = 9222
wait_until = "load"
timeout_secs = 30
```

### Environment overrides

| Variable | Meaning |
|----------|---------|
| `CORTEX_BROWSER_ENABLED` | `1`/`0` |
| `CORTEX_BROWSER_BACKEND` | `obscura` / `chrome` / `custom` |
| `CORTEX_CDP_URL` | Explicit WebSocket URL |
| `CORTEX_CDP_DISCOVERY_URL` | HTTP discovery URL |
| `CORTEX_CDP_HOST` / `CORTEX_CDP_PORT` | Host/port defaults |

## Tools

| Tool | Description |
|------|-------------|
| `browser_navigate` | Go to URL |
| `browser_evaluate` | Run JS expression |
| `browser_snapshot` | URL + title + body text |
| `browser_content` | HTML or text dump |
| `browser_click` | Click CSS selector |
| `browser_close` | Disconnect session |

## Skill

Use `--skills browser` (or natural language that matches tags like `obscura`, `scrape`, `headless`) so the agent exposes browser tools.

## Alternatives

1. **Obscura MCP** — `obscura mcp` + Cortex MCP config (`docs/mcp.md`)
2. **Playwright/Puppeteer MCP servers** — also via MCP
3. **Chrome CDP** — set `backend = "chrome"`

Cortex does not bundle a browser binary; it attaches to whatever CDP endpoint you configure.
