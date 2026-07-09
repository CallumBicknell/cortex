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
| `Enter` | Send message |
| `Ctrl+J` | Newline in composer |
| `Ctrl+B` | Toggle sessions list |
| `Ctrl+Y` | Toggle YOLO |
| `Ctrl+C` | Cancel run / quit if idle |
| `Ctrl+L` | Reset scroll to bottom |
| `PgUp` / `PgDn` | Scroll transcript |
| `Esc` | Cancel run or clear input |
| `/help` | Command list |
| `/new` | New session |
| `/sessions` | Open sessions |
| `/yolo` | Toggle auto-approve |
| `/quit` | Exit |

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
- Diff viewer / file tree
- Token cost from provider usage fields
