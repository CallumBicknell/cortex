# Cortex

An operating system for AI agents.

Cortex is a production-grade open-source agent runtime, harness, and loop engine designed to execute autonomous AI agents with reliability, observability, durability, extensibility, and high performance.

## Features

- **Deterministic event-driven architecture** with explicit state transitions (Observe → Plan → Execute → Verify → Reflect → Update Memory → Checkpoint → Continue?)
- **Plugin architecture** for models, tools, memory, storage, verification, policies, and more
- **Durable execution** with checkpoints, replay, and crash recovery (inspired by Temporal)
- **Observability** via structured logging, metrics, tracing, and event replay
- **Provider-agnostic** model abstraction (OpenAI, Anthropic, Gemini, Ollama, etc.)
- **Technology stack**: Rust core (Tokio, Axum, SQLx, SQLite/PostgreSQL), Python SDK (Pydantic, AsyncIO), future TypeScript SDK
- **Built-in tools**: filesystem, shell, Docker, Git, GitHub, HTTP, browser, Python execution, database, MCP, web search
- **SDKs**: Python as primary, TypeScript planned
- **Interfaces**: REST API, WebSocket, gRPC, CLI

## Repository Structure

- `crates/cortex-core` – Core kernel, event bus traits, and basic types
- `crates/cortex-events` – Concrete event types
- `crates/cortex-runtime` – Runtime implementation (kernel, scheduler, loop)
- `sdks/python` – Python SDK for writing agents and tools
- (Future) `sdks/typescript` – TypeScript SDK

## Getting Started

### Prerequisites

- Rust toolchain (stable)
- Python 3.9+

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Using the Python SDK

```python
from cortex import tool

@tool
def search(query: str) -> str:
    """Search the web for a query."""
    # Implementation here
    return f"Results for {query}"

# The tool can then be used by an agent running on the Cortex runtime.
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## Disclaimer

This is an early-stage project. APIs are subject to change.