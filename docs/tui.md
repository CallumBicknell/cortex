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
| `Enter` | Send message, or **queue** it if the agent is still thinking; accept completion when popup is open |
| Paste | Middle-click / terminal paste into composer (bracketed paste) |
| `Tab` | Accept autocomplete (`/skill` or `@path`) |
| `↑` / `↓` | Input history (previous prompts); when completion is open, move selection |
| Type while thinking | Composer stays editable; Enter queues the next turn |
| `Ctrl+J` | Newline in composer |
| `Ctrl+B` | Toggle sessions list |
| `Ctrl+Y` | Toggle YOLO |
| `Ctrl+C` | Cancel run / quit if idle |
| `Ctrl+L` | Reset scroll to bottom |
| `PgUp` / `PgDn` | Scroll transcript (up = older, down = newer; pin to bottom when at end) |
| Mouse drag | **Select text** in the conversation (terminal native copy — mouse capture is off on purpose) |
| `Ctrl+O` / `/copy` | Copy the last assistant reply to the system clipboard |
| `Ctrl+L` | Jump transcript to bottom |
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

### `/browser` and CDP

`/browser visit https://example.com …` selects browser tools. If nothing is listening
on the CDP port, Cortex **auto-starts** `obscura serve` (or Chrome when
`backend = "chrome"`) when the binary is on `PATH` and the host is loopback.

```bash
cortex chat
# ❯ /browser open https://example.com and summarise the title
```

Disable with `auto_start = false` in `browser.toml`. Failed tools show error text
in the activity line. See [`docs/browser.md`](browser.md).

## Behaviour

- Streams assistant tokens when the provider supports it
- Live tool / sub-agent activity under the stream
- Run summary in the status bar (`turns · tools · ms`)
- Sessions persist to the same SQLite DB as `cortex run`
- Project instructions (`AGENTS.md` / `.cortex/instructions.md`) injected automatically
- Logs never write to the terminal during `chat`/`tui`/`setup` (they paint over
  the alternate screen). Instead they append to `~/.cortex/logs/cortex.log`.
  Use `--verbose` only if you want stderr logs (can still corrupt the UI).
- Starts a **fresh session** each launch; open prior ones with `Ctrl+B`

## Not yet

- Inline tool-approval modal
- Click-to-open paths / in-app multi-selection ranges
- Fuzzy multi-root `@` search across large monorepos
- Diff viewer / file tree
- Token cost from provider usage fields
