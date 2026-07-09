# Follow-ups (post Phase 16)

Features landed after the M0–M16 MVP.

## Dynamic / external plugins

Directory plugins with `plugin.toml` + command tools (no recompile).

```
plugins/example_echo/plugin.toml
.cortex/plugins/<id>/plugin.toml
```

Auto-discovered when `auto_discover = true` in `config/plugins.toml`.

```toml
[[plugins]]
id = "my_ext"
path = "plugins/my_ext"
```

See [`plugin-system.md`](plugin-system.md).

## Bubblewrap shell

When `bwrap` is installed and `shell_use_bubblewrap = true` (default), `shell`
runs under bubblewrap with **no network** and workspace bind-mount.

```bash
CORTEX_SHELL_BWRAP=0  # force disable
```

## Streaming API

```http
POST /v1/runs/stream
Content-Type: application/json

{"prompt":"hello","yolo":true}
```

SSE events (JSON in `data`): `started`, `session`, `running`, `tool`, `done`, `error`.

## Sub-agent events

Parent event bus receives:

- `agent.subagent.started`
- `agent.subagent.finished`

Child runs also publish normal loop events when a bus is attached.

## Code intelligence (LSP-lite)

| Tool | Role |
|------|------|
| `code_outline` | File outline |
| `workspace_symbols` | Workspace symbol search |
| `code_definition` | Find definitions by name |

Rust + Python via tree-sitter. Not a full language server (no diagnostics/hover).

## Self-evolving skills

See [`evolving-skills.md`](evolving-skills.md).
