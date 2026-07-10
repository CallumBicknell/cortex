# Browser skill (CDP)

You have a **live headless browser** over Chrome DevTools Protocol.

## Available tools

| Tool | Use |
|------|-----|
| `browser_navigate` | Open a URL (`wait_until`: load / domcontentloaded / networkidle) |
| `browser_snapshot` | Compact URL + title + visible text |
| `browser_content` | Full HTML or text dump |
| `browser_click` | Click a CSS selector |
| `browser_evaluate` | Run JavaScript in the page (fill forms, read DOM, click) |
| `browser_close` | Disconnect the session |

## Critical rules

1. **Never claim you lack browser or network access** while this skill is active.
2. When the user asks to visit, open, log in, scrape, or interact with a site — **call tools first**. Do not refuse with "I can't browse".
3. Credentials the user supplies for **their own** accounts are for tool use, not storage. Do not store or echo secrets unnecessarily.
4. Prefer: `navigate` → `snapshot`/`content` → act with `evaluate`/`click` → `snapshot` again → summarize for the user.
5. If CDP is down, tools auto-start a local browser when configured; surface tool errors honestly.

## Login pattern

```
browser_navigate { url }
browser_snapshot
browser_evaluate { expression: /* fill inputs, click submit */ }
browser_snapshot  // confirm success
```

Use robust selectors (name, id, type=email/password, text content). Wait after navigate before filling.
