# Browser automation (CDP)

Cortex drives headless browsers over the **Chrome DevTools Protocol**.
That means you can use:

| Backend | How to run | Default endpoint |
|---------|------------|------------------|
| **[Obscura](https://github.com/h4ckf0r0day/obscura)** | `obscura serve --port 9222` | `ws://127.0.0.1:9222/devtools/browser` |
| **Chrome / Chromium** | `chromium --remote-debugging-port=9222 --headless=new` | discovered via `http://127.0.0.1:9222/json/version` |
| **Custom** | any CDP-compatible browser | set `cdp_url` |

## Quick start (Obscura)

If `obscura` (or Chrome/Chromium for `backend = "chrome"`) is on your `PATH`,
Cortex **auto-starts** it the first time a browser tool needs CDP and nothing is
listening on `host:port` (default `127.0.0.1:9222`). You can also start it yourself:

```bash
# Optional — Cortex will do this for you when auto_start = true
obscura serve --port 9222

cortex chat
# ❯ /browser open https://example.com and summarise the title

# or
cortex run "Open https://example.com and tell me the title" --skills browser --yolo
```

Disable auto-start with `auto_start = false` in `browser.toml` or
`CORTEX_BROWSER_AUTO_START=0`. Auto-start only runs for **loopback** hosts.

Docker (manual, no auto-start from inside Cortex unless the binary is local):

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
auto_start = true
auto_start_timeout_secs = 15
```

### Environment overrides

| Variable | Meaning |
|----------|---------|
| `CORTEX_BROWSER_ENABLED` | `1`/`0` |
| `CORTEX_BROWSER_BACKEND` | `obscura` / `chrome` / `custom` |
| `CORTEX_BROWSER_AUTO_START` | `1`/`0` — spawn local browser if CDP is down |
| `CORTEX_BROWSER_AUTO_START_TIMEOUT_SECS` | Wait for auto-start readiness |
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

Cortex does not bundle a browser binary; it attaches to a CDP endpoint (and can
auto-start Obscura/Chrome when installed locally).
