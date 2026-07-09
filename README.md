# Cortex

An operating system for AI agents — with a **coding agent loop** and
**smart-contract security** skills (Solidity / Foundry audits, vuln finding).

Cortex is an open-source **agent runtime**: durable, observable, provider-agnostic execution for autonomous agents. The LLM is one component. **The runtime is the product.** Web3 tooling can be extended via [skills.eth.sh](https://skills.eth.sh/) MCP/skill packs (see [`docs/web3-security.md`](docs/web3-security.md)).

## Status

**v0.2.0** — agent OS MVP **plus** coding agent + smart-contract security arc
(Phases 17–25). See [`CHANGELOG.md`](CHANGELOG.md) and [`docs/roadmap.md`](docs/roadmap.md).

| Area | Status |
|------|--------|
| Kernel / event bus / models / providers | Implemented |
| Agent loop (LLM ↔ tools, budgets, sub-agents) | Implemented |
| CLI (`run` / `chat` / `init` / …) + TUI + HTTP API | Implemented |
| Tools (fs, shell, git, http, browser CDP, patch, …) | Implemented |
| Skills (coding, Solidity, sc_security, sc_xray, …) | Implemented |
| Multi-lens audits (`audit_lenses`) + audit reports | Implemented |
| Tree-sitter outlines (Rust, Python, **Solidity**) | Implemented |
| MCP **stdio + Streamable HTTP** | Implemented |
| `skills import` + skills.eth.sh recipes | Implemented |
| Parallel read tools, `--plan`, `--verify` | Implemented |
| SQLite memory / evals / CI/CD | Implemented |
| Full LSP / cdylib plugins / multi-tenant Postgres | Planned (later) |

### Quick SC security path

```bash
cortex skills select "audit this vault for reentrancy"
cortex run "Audit examples/foundry-vault" --skills sc_security,solidity --yolo
cortex parse outline examples/foundry-vault/src/VulnerableVault.sol
# docs: docs/web3-security.md · docs/web3-recipes.md
```

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
  cortex-mcp/       # MCP stdio + Streamable HTTP → Tool adapters
  cortex-plugins/   # In-process plugin host + builtins
  cortex-parse/     # Tree-sitter outlines (Rust/Python/Solidity)
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

### Install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/CallumBicknell/cortex/main/scripts/install.sh | sh
cortex doctor
export OPENAI_API_KEY=…   # or use ollama / mock in ~/.cortex/models.toml
cd my-project && cortex run "hello"
```

Creates **`~/.cortex/`** (global config + fallback DB). Optional project overrides: `cortex init` → `<repo>/.cortex/`.

Full details: [`docs/install.md`](docs/install.md).

### Prerequisites (from source)

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

### CLI

```bash
cortex setup                  # ~/.cortex (auto-created on first use too)
cortex doctor
cortex init                   # project .cortex/ (optional)

# Offline mock provider (default in models.toml)
cortex tools list
cortex models list
cortex run "What is Cortex?" --json

# In any project:
cortex run "Summarize this repo" --model ollama --yolo
cortex run "Audit this Foundry project for reentrancy" --skills sc_security,solidity --yolo
cortex run "Refactor carefully" --plan --verify --yolo
cortex skills import ./path/to/SKILL.md --dry-run
cortex chat --model openai

# Sessions (project .cortex/data or ~/.cortex/data)
cortex sessions list
cortex sessions show <session-id>
cortex sessions resume <session-id>
cortex sessions export <session-id> -o session.json

# Workspace awareness
cortex workspace info
cortex workspace map

# Skills (capability packs)
cortex skills list
cortex skills select "audit solidity with forge"
cortex run "fix cargo test" --skills rust,testing
cortex security show
cortex plugins list
cortex memory index
cortex memory search "agent loop"
cortex parse outline crates/cortex-runtime/src/agent_loop.rs
cortex tui
cortex serve --bind 127.0.0.1:8080
cortex eval run

# From a clone without installing:
# cargo run -p cortex-cli -- <args>

# Browser via CDP (Obscura default — start `obscura` or Chrome first)
# cortex run "Open https://example.com and report the title" --skills browser --yolo
```

See [`docs/install.md`](docs/install.md), [`examples/hello_agent.md`](examples/hello_agent.md), [`docs/skills.md`](docs/skills.md), [`docs/security.md`](docs/security.md), [`docs/browser.md`](docs/browser.md), [`docs/plugin-system.md`](docs/plugin-system.md), [`docs/memory.md`](docs/memory.md), [`docs/parse.md`](docs/parse.md), [`docs/tui.md`](docs/tui.md), [`docs/api.md`](docs/api.md), [`docs/hardening.md`](docs/hardening.md), [`docs/evolving-skills.md`](docs/evolving-skills.md), [`docs/follow-ups.md`](docs/follow-ups.md), and [`docs/ci.md`](docs/ci.md).

### Configuration

| Location | Purpose |
|----------|---------|
| `~/.cortex/models.toml` | User-global providers / default model |
| `<project>/.cortex/` | Project overrides, audits, local DB |
| [`config/`](config/) | Monorepo defaults (dev + embedded into binary) |

```bash
export CORTEX_HOME=~/.cortex-work     # alternate home profile
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
