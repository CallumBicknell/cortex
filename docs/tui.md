# Terminal UI

Cortex ships a **ratatui** terminal UI for interactive agent sessions.

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
- Each Enter starts an `AgentLoop` turn in a background task; the transcript and tool log update when it finishes.
- Sessions persist to the same SQLite DB as `cortex chat` / `cortex run`.
- Rolling summaries from Phase 12 still apply for long sessions.

## Not yet

- Streaming token deltas into the transcript
- Inline tool-approval modal
- Mouse support
- Split-pane file browser / diff viewer
- Theme configuration

## Library

```rust
// cortex-tui::run(TuiHost { ... })
```

Host is prepared by the CLI after `AppContext::bootstrap`.
