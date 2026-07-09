# Hardening, sub-agents, and evals

Phase 16 adds **run budgets**, **nested sub-agents**, and a **fixture eval harness**.

## Run budgets

`AgentLoopConfig` limits:

| Field | Default | Meaning |
|-------|---------|---------|
| `max_turns` | 32 | LLM turns |
| `max_run_secs` | 600 | Wall-clock timeout (`0` = unlimited) |
| `max_tool_calls_per_turn` | 16 | Fail closed if a model emits more |
| `max_subagent_depth` | 2 | Nesting limit for `spawn_subagent` |
| `subagent_depth` | 0 | Current depth (set by runtime) |

Timeouts return `RuntimeError::RunTimeout`. Excess tool calls return `TooManyToolCalls`.

## Path / shell hardening

`cortex-security` helpers:

- `reject_absolute_path` / `safe_join` — block `..` and absolute escapes
- `bubblewrap_available` / `bubblewrap_shell_prefix` — optional `bwrap` isolation (not wired into shell by default; detect + document for operators)

Existing protections still apply: workspace sandbox, shell deny patterns, env scrub, HTTP host blocks.

## Sub-agents

Tool: **`spawn_subagent`**

```json
{
  "prompt": "Research how models.toml is loaded and summarize",
  "max_turns": 6,
  "tools": ["read_file", "glob_files", "list_dir"]
}
```

Behaviour:

- Nested `AgentLoop` with depth check
- Child never receives `spawn_subagent` (recursion guard)
- Shorter wall budget (≤300s)
- Summarization off by default for speed
- Registered on `cortex run` / `chat` via `tools_with_subagent`

Library:

```rust
use cortex_runtime::{run_subagent, SubAgentOptions, tools_with_subagent};
```

## Evals

Fixtures live under `evals/*.toml`:

```toml
id = "hello"
prompt = "Say hello"
expect_contains = ["hello"]

[[mock]]
type = "text"
content = "hello from cortex eval"
```

CLI:

```bash
cargo run -p cortex-cli -- eval list
cargo run -p cortex-cli -- eval run
cargo run -p cortex-cli -- eval run --dir evals --json
```

Exit code `1` if any case fails (CI-friendly).

Crate: `cortex-eval` (`run_suite`, `run_fixture`, `load_fixture`).

## Not yet

- Automatic bubblewrap wrapping for every shell invocation
- Firejail / seccomp profiles
- Parallel multi-agent orchestration DAG
- Live-provider eval cloud runner
- Streaming sub-agent event merge into parent bus
