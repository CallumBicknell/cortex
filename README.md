# Cortex

An operating system for AI agents.

Cortex is an open-source **agent runtime**: durable, observable, provider-agnostic execution for autonomous agents. The LLM is one component. **The runtime is the product.**

## Status

**Early development (v0.1.0).** Phases 0–5 complete: kernel, models, providers, tools, agent loop, and a usable CLI. Still missing: durable sessions, TUI, skills/MCP, and richer security.

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
| Tools (fs, shell, git, http) + permissions | Implemented |
| Agent loop (LLM ↔ tools, events) | Implemented |
| CLI (`cortex run` / `chat` / `init`) | Implemented |
| Unit / golden serde / HTTP mock tests | Implemented |
| SQLite sessions / checkpoints | Planned (Phase 6) |
| Skills / plugins / MCP | Planned (Phases 8–11) |
| Python SDK | Stub only |
| TUI / HTTP API | Planned (later) |

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
  cortex-tools/     # Tool trait, registry, executor, fs/shell/git/http
  cortex-core/      # Kernel, config, event bus, service registry, cancel
  cortex-events/    # Lifecycle re-exports + agent loop events
  cortex-runtime/   # Kernel facade + AgentLoop
  cortex-cli/       # `cortex` binary
config/             # Default TOML configuration
examples/           # Usage walkthroughs
scripts/            # smoke_agent.sh
sdks/python/        # Python SDK stubs (not wired to runtime yet)
docs/               # Architecture and design notes
```

## Getting started

### Prerequisites

- Rust stable (see `rust-toolchain.toml`)
- Optional: Python 3.9+ for the SDK stubs

### Build & test

```bash
cargo build
cargo test
cargo clippy --workspace
./scripts/smoke_agent.sh
```

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
```

See [`examples/hello_agent.md`](examples/hello_agent.md).

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
| M6+ | Persistence, skills, security, MCP, plugins |

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
