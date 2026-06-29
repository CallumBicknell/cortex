You are a principal systems architect and senior Rust/Python engineer.

Your task is to design and implement a production-grade open-source Agent Runtime, Harness, and Loop Engine.

This is NOT another LangGraph clone, CrewAI clone, or wrapper around an existing framework.

The objective is to build an "Agent Operating System" that executes autonomous AI agents with reliability, observability, durability, extensibility, and high performance.

========================

Core philosophy

========================

The LLM is only one component.

The runtime is the product.

Everything should revolve around:

• deterministic execution
• event-driven architecture
• explicit state transitions
• durable execution
• checkpoints
• replayability
• observability
• verification
• extensibility
• plugin architecture

The runtime should function similarly to an operating system for AI agents.

========================

Technology stack

========================

Core Runtime:
- Rust (stable)
- Tokio
- Axum
- SQLx
- SQLite by default
- PostgreSQL support
- Serde
- Tracing
- OpenTelemetry
- UUID
- Chrono

SDK:
- Python
- Pydantic
- AsyncIO

Future SDK:
- TypeScript

========================

Architecture

========================

Implement clean modules including:

Kernel
Scheduler
Loop Engine
Planner
Context Manager
Memory Manager
Tool Registry
Plugin Manager
Model Provider Manager
Session Manager
Checkpoint Manager
Artifact Manager
Verification Engine
Policy Engine
Approval Engine
Metrics
Tracing
Configuration

The runtime must communicate internally using an event bus.

========================

Loop

========================

Implement a deterministic execution loop.

Observe

↓

Plan

↓

Execute

↓

Verify

↓

Reflect

↓

Update Memory

↓

Checkpoint

↓

Continue?

Never implement a simple while(true) loop.

Use explicit state machines and event-driven transitions.

========================

Features

========================

Implement:

- model abstraction
- provider abstraction
- plugin system
- MCP integration
- filesystem tools
- shell execution
- docker execution
- browser automation
- web search
- HTTP tools
- artifact storage
- replayable sessions
- checkpoints
- long-term memory
- working memory
- session memory
- verification pipeline
- retry policies
- rollback
- human approval hooks
- tracing
- metrics
- structured logging
- WebSocket streaming
- REST API
- CLI
- configuration system

========================

Engineering standards

========================

Everything must be:

typed

async

modular

unit tested

integration tested

benchmarkable

documented

dependency injected

extensible

========================

Output

========================

Do NOT immediately write code.

Instead:

1. Produce a complete architecture document.
2. Design every subsystem.
3. Explain design decisions and trade-offs.
4. Define the project structure.
5. Design interfaces and traits.
6. Define events.
7. Define state objects.
8. Define plugin APIs.
9. Design persistence.
10. Design loop execution.
11. Design checkpoints.
12. Design observability.
13. Design verification.
14. Design configuration.
15. Produce a complete development roadmap.
16. Break implementation into small milestones.
17. Only after the architecture is finalized should implementation begin.

Prioritize maintainability, extensibility, and long-term production readiness over short-term simplicity.
