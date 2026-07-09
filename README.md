# Cortex

An operating system for AI agents.

Cortex is an open-source **agent runtime**: durable, observable, provider-agnostic execution for autonomous agents. The LLM is one component. **The runtime is the product.**

## Status

**Early development (v0.1.0 MVP).** Phases 0–16 + follow-ups shipped on the feature branch: agent loop, tools, skills, memory, plugins, TUI, HTTP API, Python SDK, evals, and CI/CD. See [`CHANGELOG.md`](CHANGELOG.md).

| Area | Status |
|------|--------|
| Kernel lifecycle | Implemented |
| In-memory event bus + history | Implemented |
| Cancellation tokens | Implemented |
| Service registry | Implemented |
| Config (TOML + env) | Implemented |
| Domain models (messages, tools, sessions) | Implemented |
| Agent event types | Implemented |
| LLM providers (OpenAI-compat, Anthropic, mock) | Implemented |
| Provider registry + `config/models.toml` | Implemented |
| Tools (fs, shell, git, http, browser/CDP) + permissions | Implemented |
| Agent loop (LLM ↔ tools, events) | Implemented |
| CLI (`cortex run` / `chat` / `init`) | Implemented |
| SQLite sessions / checkpoints | Implemented |
| Workspace map + context budgets | Implemented |
| Skills + prompts (capability packs) | Implemented |
| Security policy + audit + redaction | Implemented |
| MCP client + docker/search/patch tools | Implemented |
| Browser tools via CDP (Obscura/Chrome) | Implemented |
| In-process plugins (`echo` demo) | Implemented |
| Rolling summaries + local vector memory | Implemented |
| Tree-sitter code outlines (Rust/Python) | Implemented |
| Terminal UI (`cortex tui`) | Implemented |
| HTTP API (`cortex serve`) | Implemented |
| Python SDK (HTTP client) | Implemented |
| Run budgets + spawn_subagent + eval harness | Implemented |
| External plugins + bubblewrap shell + SSE API | Implemented |
| Workspace symbols + evolving skills | Implemented |
| Unit / golden serde / HTTP mock tests | Implemented |
| Full LSP / cdylib plugins / token streaming | Planned (later) |

## Design principles

- Provider-independent core (no hard dependency on a single LLM vendor)
- Uniform `Tool` / `Provider` / `Plugin` interfaces (as they land)
- Event-driven, serializable state
- Cancellation for long-running work
- SQLite first; headless-first (no dashboard required)
- Skills as capability packs — not hard-coded “modes”

See [`CONSTRAINTS.md`](CONSTRAINTS.md), [`SPEC.md`](SPEC.md), [`VISION.md`](VISION.md), and [`docs/`](docs/).

## Repository structure

```text
crates/
  cortex-common/    # Errors, typed IDs
  cortex-models/    # Message, ToolCall, Session, Plan, Task, Artifact
  cortex-llm/       # Provider trait, OpenAI-compat, Anthropic, mock, registry
  cortex-tools/     # Tool trait, registry, executor, fs/shell/git/http/browser
  cortex-core/      # Kernel, config, event bus, service registry, cancel
  cortex-events/    # Lifecycle re-exports + agent loop events
  cortex-runtime/   # Kernel facade + AgentLoop
  cortex-memory/    # SQLite sessions, checkpoints, summaries, vectors
  cortex-workspace/ # Root detect, ignore, project, repo map
  cortex-context/   # Token budgets, history compression
  cortex-prompts/   # Markdown prompts + templates
  cortex-skills/    # Skill packs (not hard-coded modes)
  cortex-security/  # Policy, redaction, approval audit
  cortex-mcp/       # MCP stdio client → Tool adapters
  cortex-plugins/   # In-process plugin host + builtins
  cortex-parse/     # Tree-sitter outlines (Rust/Python)
  cortex-tui/       # ratatui interactive UI
  cortex-api/       # axum HTTP API
  cortex-eval/      # Fixture-driven agent evals
  cortex-cli/       # `cortex` binary
config/             # Default TOML (models, security, mcp, browser, plugins)
evals/              # Eval fixtures (TOML)
prompts/            # System + skill markdown
migrations/         # SQL schema
examples/           # Usage walkthroughs
scripts/            # smoke_agent.sh
sdks/python/        # Python SDK (HTTP client for cortex serve)
docs/               # Architecture and design notes
```

## Getting started

### Prerequisites

- Rust stable (see `rust-toolchain.toml`)
- Optional: Python 3.9+ for the SDK stubs

### Build & test

```bash
cargo build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# Full local CI (fmt, clippy, tests, evals, smoke, Python SDK)
./scripts/ci_local.sh
# or: make ci
```

See [`docs/ci.md`](docs/ci.md) for GitHub Actions, releases, Docker, and Dependabot.

### CLI (usable now)

```bash
# Offline mock provider (default in config/models.toml)
cargo run -p cortex-cli -- tools list
cargo run -p cortex-cli -- models list
cargo run -p cortex-cli -- run "What is Cortex?" --json

# In any project:
cargo run -p cortex-cli -- init
cargo run -p cortex-cli -- run "Summarize this repo" --model ollama --yolo
cargo run -p cortex-cli -- chat --model openai

# Sessions (persisted under .cortex/data/cortex.db)
cargo run -p cortex-cli -- sessions list
cargo run -p cortex-cli -- sessions show <session-id>
cargo run -p cortex-cli -- sessions resume <session-id>
cargo run -p cortex-cli -- sessions export <session-id> -o session.json

# Workspace awareness
cargo run -p cortex-cli -- workspace info
cargo run -p cortex-cli -- workspace map

# Skills (capability packs)
cargo run -p cortex-cli -- skills list
cargo run -p cortex-cli -- skills select "audit solidity with forge"
cargo run -p cortex-cli -- run "fix cargo test" --skills rust,testing
cargo run -p cortex-cli -- security show
cargo run -p cortex-cli -- plugins list
cargo run -p cortex-cli -- memory index
cargo run -p cortex-cli -- memory search "agent loop"
cargo run -p cortex-cli -- parse outline crates/cortex-runtime/src/agent_loop.rs
cargo run -p cortex-cli -- tui
cargo run -p cortex-cli -- serve --bind 127.0.0.1:8080
cargo run -p cortex-cli -- eval run

# Browser via CDP (Obscura default — start `obscura` or Chrome first)
# cargo run -p cortex-cli -- run "Open https://example.com and report the title" \
#   --skills browser --yolo
```

See [`examples/hello_agent.md`](examples/hello_agent.md), [`docs/skills.md`](docs/skills.md), [`docs/security.md`](docs/security.md), [`docs/browser.md`](docs/browser.md), [`docs/plugin-system.md`](docs/plugin-system.md), [`docs/memory.md`](docs/memory.md), [`docs/parse.md`](docs/parse.md), [`docs/tui.md`](docs/tui.md), [`docs/api.md`](docs/api.md), [`docs/hardening.md`](docs/hardening.md), [`docs/evolving-skills.md`](docs/evolving-skills.md), [`docs/follow-ups.md`](docs/follow-ups.md), and [`docs/ci.md`](docs/ci.md).

### Configuration

Default config: [`config/default.toml`](config/default.toml).

```bash
# Environment overrides
export CORTEX_LOG_LEVEL=debug
export CORTEX_EVENT_HISTORY_SIZE=2048
```

Load from Rust:

```rust
use cortex_core::Config;

let cfg = Config::from_file("config/default.toml")?;
// or
let cfg = Config::from_env();
```

### Minimal kernel example

```rust
use cortex_core::{Config, Kernel};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kernel = Arc::new(Kernel::with_config(Config::default()));
    let k = Arc::clone(&kernel);
    let handle = tokio::spawn(async move { k.start().await });

    tokio::time::sleep(Duration::from_millis(50)).await;
    kernel.stop();
    handle.await??;
    Ok(())
}
```

## Roadmap (milestones)

| Milestone | Outcome |
|-----------|---------|
| M0 Stabilize | Compiling kernel + real bus ✓ |
| M1 Models/events | Shared message/session types ✓ |
| M2 Providers | Chat + mock + OpenAI-compatible ✓ |
| M3 Tools | fs + shell + registry ✓ |
| M4 Agent loop | Multi-step tool use ✓ |
| M5 CLI | `cortex run` / `cortex chat` ✓ |
| M6 Durable sessions | SQLite + checkpoints ✓ |
| M7 Context-aware | Repo map + token budgets ✓ |
| M8 Skills | Capability packs + prompts ✓ |
| M9 Security | Policy + audit + redaction ✓ |
| M10 MCP + tools | MCP, docker, search, patch ✓ |
| M11+ | Plugins, TUI, API |

Full task list: [`TASKS.md`](TASKS.md).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`AGENTS.md`](AGENTS.md).

AI agents: prefer small vertical slices; do not invent APIs that are not in `TASKS.md` / the plan.

## License

Licensed under either of

- Apache License, Version 2.0
- MIT license

at your option.

## Disclaimer

This is an early-stage project. APIs will change. Documentation under `docs/` may describe future design; trust **this README’s status table** and the code for what exists today.
