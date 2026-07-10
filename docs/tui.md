# Chat TUI

`cortex chat` opens a **Claude Code–style** full-screen terminal UI (also available as `cortex tui`).

```bash
cortex chat
cortex chat --model proxy
cortex chat --no-yolo          # tools may block without approval UX
cortex chat --plain            # old line-based REPL
```

## Layout

```text
 cortex  ·  proxy · auto  ·  ~/project  ·  yolo

 You
 hello

 Cortex
 streaming reply…

   · read_file  ok

 ┌ message ─────────────────────────────┐
 │ ❯ your input ▌                       │
 └──────────────────────────────────────┘
 ready  ·  ↵ send  ^J newline  ^B sessions …
```

Conversation-first: single column, multi-line messages, live token stream, subtle tool chips. Sessions open as a modal (Ctrl+B), not a permanent sidebar.

## Keys

| Key | Action |
|-----|--------|
| `Enter` | Send message (or accept completion when popup is open) |
| `Tab` | Accept autocomplete (`/skill` or `@path`) |
| `↑` / `↓` | Move completion selection |
| `Ctrl+J` | Newline in composer |
| `Ctrl+B` | Toggle sessions list |
| `Ctrl+Y` | Toggle YOLO |
| `Ctrl+C` | Cancel run / quit if idle |
| `Ctrl+L` | Reset scroll to bottom |
| `PgUp` / `PgDn` | Scroll transcript |
| `Esc` | Dismiss completion, cancel run, or clear input |
| `/help` | Command list |
| `/skills` | List skill packs |
| `/new` | New session |
| `/sessions` | Open sessions |
| `/yolo` | Toggle auto-approve |
| `/quit` | Exit |

## `/skills` and `@paths`

Claude Code–style composer mentions:

| Syntax | Effect |
|--------|--------|
| `/git fix the commit` | Activates the `git` skill for that turn (plus always-on packs) |
| `/solidity` + message | Force-select a skill; Tab completes skill ids and meta commands |
| `@src/main.rs` | Inlines file contents into the agent prompt (transcript keeps `@…`) |
| `@crates/cortex-tui/` | Attaches a directory listing |
| `fix @a.rs with /web` | Combine path attachments and skill slash tokens |

Type `/` or `@` to open the completion popup. **Tab** or **Enter** accepts; **Esc** dismisses. After accepting a directory (`@src/`), keep typing or Tab again to nest.

### `/browser` needs a live CDP endpoint

`/browser visit https://example.com …` selects browser tools, but **`browser_navigate` fails if nothing is serving CDP** (default `ws://127.0.0.1:9222`).

```bash
# terminal 1
obscura serve --port 9222

# terminal 2
cortex chat
# ❯ /browser open https://example.com and summarise the title
```

Failed tools now show the error text in the activity line (e.g. `CDP connect failed … Start a browser first`). See [`docs/browser.md`](browser.md).

## Behaviour

- Streams assistant tokens when the provider supports it
- Live tool / sub-agent activity under the stream
- Run summary in the status bar (`turns · tools · ms`)
- Sessions persist to the same SQLite DB as `cortex run`
- Project instructions (`AGENTS.md` / `.cortex/instructions.md`) injected automatically
- Logs default to **error** for all crates during `chat`/`tui` (set before
  tracing init) so INFO lines never paint over the alternate screen. Use
  `--verbose` or `RUST_LOG=info` to debug.
- Starts a **fresh session** each launch; open prior ones with `Ctrl+B`

## Not yet

- Inline tool-approval modal
- Mouse selection / click-to-open paths
- Fuzzy multi-root `@` search across large monorepos
- Diff viewer / file tree
- Token cost from provider usage fields
