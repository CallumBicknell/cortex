# Terminal UI

Cortex ships a **ratatui** terminal UI for interactive agent sessions.

While a run is active the transcript shows a **live streaming draft** (token
deltas when the provider supports streaming). The tools/log pane updates with
tool and sub-agent activity; a one-line **run summary** (turns, tool ok/err,
duration) is appended when the run finishes.

```bash
cargo run -p cortex-cli -- tui
cargo run -p cortex-cli -- tui --model ollama --max-turns 24
cargo run -p cortex-cli -- tui --no-yolo   # tools may block without approval UX yet
```

## Layout

| Pane | Content |
|------|---------|
| Header | Model, workspace, YOLO flag, run indicator |
| Sessions | Recent sessions (↑/↓, Enter to open) |
| Transcript | Conversation messages |
| Tools / log | Recent tool results |
| Input | Prompt editor |
| Status | Run status + key help |

## Keys

| Key | Action |
|-----|--------|
| `Enter` | Send message (input focused) or open session (list focused) |
| `Tab` | Toggle focus: input ↔ session list |
| `i` | Focus input |
| `n` | New session |
| `r` | Reload session list |
| `y` | Toggle YOLO (auto-approve tools) |
| `q` / `Esc` | Quit (or cancel if a run is active) |
| `Ctrl-C` | Cancel current run (or quit if idle) |
| Backspace | Edit input |

## Behaviour

- Default **yolo=true** so tools work without a modal approver (TUI approval UI is not built yet). Use `--no-yolo` only if you accept denials for ask-mode tools.
- Each Enter starts an `AgentLoop` turn in a background task with **token streaming** when the provider supports it.
- Tools/log pane updates live for tool requests/completions and sub-agent start/finish.
- When the run ends, a summary line is logged (`turns · tools ok/err · ms`).
- Sessions persist to the same SQLite DB as `cortex chat` / `cortex run`.
- Rolling summaries from Phase 12 still apply for long sessions.
- Project instructions (`AGENTS.md` / `.cortex/instructions.md`) are injected like the CLI.

## Not yet

- Inline tool-approval modal
- Mouse support
- Split-pane file browser / diff viewer
- Theme configuration
- Token / cost accounting from provider usage fields

## Library

```rust
// cortex-tui::run(TuiHost { ... })
```

Host is prepared by the CLI after `AppContext::bootstrap`.
