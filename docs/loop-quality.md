# Agent loop quality (plan, parallel tools, verify)

## Safe parallel tool execution

When the model requests multiple tools in one turn, Cortex runs **consecutive
read-only tools in parallel** (`join_all`), then keeps mutating tools serial:

| Parallel-safe (examples) | Always serial |
|--------------------------|---------------|
| `read_file`, `list_dir`, `glob_files` | `write_file`, `edit_file`, `apply_patch` |
| `code_outline`, `workspace_symbols` | `shell`, `git_add`, `git_commit` |
| `git_status`, `git_diff`, `git_log` | `spawn_subagent`, `audit_lenses` |
| `memory_search`, `web_search` | `http_request`, browser navigate/click |

Implementation: `ToolExecutor::execute_all` in `cortex-tools`.

## Plan mode

```bash
cortex run "Refactor the parser" --plan --yolo
cortex chat --plan
```

Injects guidance: outline a short **Plan** before big edits; prefer reads first.

## Verify after writes

```bash
# Auto-detect test command from project (cargo test / forge test / …)
cortex run "Fix the failing unit test" --verify --yolo

# Explicit command
cortex run "…" --verify-cmd "cargo test -p cortex-tools" --yolo
```

After a successful file mutation (`write_file` / `edit_file` / `apply_patch` /
`write_audit_report`), the loop runs one extra `shell` tool with the verify
command and appends the output as a tool result for the next LLM turn.

**Caution:** use carefully on large monorepos; prefer a targeted `--verify-cmd`.
