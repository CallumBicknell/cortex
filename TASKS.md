# TASKS.md

> Master development backlog for Cortex.
>
> Tasks should be completed in roughly this order unless priorities change.
>
> Rules:
>
> * One logical task = one issue / one branch / one or more related commits.
> * Check items off instead of deleting them.
> * Add new ideas to `IDEAS.md` first, then promote them here once approved.
> * Keep tasks small and reviewable.
> * Every completed task should include tests and documentation where applicable.

---

# Legend

* [ ] Not Started
* [x] Complete
* [~] In Progress
* [!] Blocked

---

# Milestone 0 — Project Foundation

## Repository

* [x] Create Rust/Go workspace
* [x] Create project structure
* [ ] Configure Git
* [ ] Configure editor settings
* [x] Configure `.gitignore`
* [x] Configure licensing
* [x] Create README
* [ ] Create CONTRIBUTING
* [ ] Create CODE_OF_CONDUCT
* [ ] Create SECURITY policy

## Development

* [x] Configure formatter
* [x] Configure linter
* [x] Configure tests
* [ ] Configure benchmarks
* [x] Configure CI
* [ ] Configure release workflow

## Documentation

* [ ] Architecture overview
* [ ] Runtime overview
* [ ] Loop design
* [ ] Event system
* [ ] Plugin system
* [ ] Roadmap

---

# Milestone 1 — Runtime Kernel

* [x] Runtime lifecycle
* [x] Startup
* [x] Shutdown
* [ ] Dependency injection
* [ ] Service registry
* [x] Configuration loader
* [ ] Runtime builder
* [ ] Health checks

---

# Milestone 2 — Event System

* [x] Typed events
* [x] Event bus
* [x] Event dispatcher
* [x] Event subscriptions
* [x] Async event handlers
* [ ] Event replay
* [ ] Event persistence

---

# Milestone 3 — Scheduler

* [ ] Scheduler
* [ ] Job queue
* [ ] Priorities
* [ ] Retry policies
* [ ] Timeouts
* [ ] Cancellation
* [ ] Parallel execution
* [ ] Worker pools

---

# Milestone 4 — Agent Loop

* [ ] Loop state machine
* [ ] Observe phase
* [ ] Plan phase
* [ ] Execute phase
* [ ] Verify phase
* [ ] Reflect phase
* [ ] Memory update
* [ ] Continue decision
* [ ] Loop metrics

---

# Milestone 5 — Model Providers

* [ ] Provider interface
* [ ] Streaming support
* [ ] Tool calling
* [ ] Structured output
* [ ] Retry handling
* [ ] Token accounting
* [ ] Cost tracking

Providers

* [ ] OpenAI
* [ ] Anthropic
* [ ] Gemini
* [ ] Ollama
* [ ] OpenRouter
* [ ] LM Studio
* [ ] vLLM

---

# Milestone 6 — Context Engine

* [ ] Prompt builder
* [ ] Content builder
* [ ] Token budgeting
* [ ] Content pruning
* [ ] Content compression
* [ ] Message history
* [ ] Artifact injection

---

# Milestone 7 — Memory

* [ ] Working memory
* [ ] Session memory
* [ ] Long-term memory
* [ ] Embeddings
* [ ] Vector search
* [ ] Memory pruning
* [ ] Memory summarisation

---

# Milestone 8 — Tools

Framework

* [ ] Tool interface
* [ ] Tool registry
* [ ] Tool permissions
* [ ] Tool sandboxing
* [ ] Tool lifecycle

Built-in Tools

* [ ] Filesystem
* [ ] Shell
* [ ] Docker
* [ ] Git
* [ ] GitHub
* [ ] HTTP
* [ ] Browser
* [ ] Python
* [ ] Database
* [ ] MCP
* [ ] Web Search

---

# Milestone 9 — Plugins

* [ ] Plugin API
* [ ] Plugin loader
* [ ] Plugin discovery
* [ ] Version compatibility
* [ ] Plugin sandbox

---

# Milestone 10 — Storage

* [ ] SQLite
* [ ] PostgreSQL
* [ ] Migrations
* [ ] Artifact storage
* [ ] Session storage
* [ ] State persistence

---

# Milestone 11 — Checkpoints

* [ ] Checkpoint creation
* [ ] Restore
* [ ] Resume
* [ ] Rollback
* [ ] Crash recovery

---

# Milestone 12 — Verification

* [ ] Output verification
* [ ] Tool verification
* [ ] Reflection
* [ ] Retry engine
* [ ] Human approval
* [ ] Policy engine

---

# Milestone 13 — Observability

* [ ] Structured logging
* [ ] Metrics
* [ ] Tracing
* [ ] Timeline
* [ ] Event viewer
* [ ] Loop replay

---

# Milestone 14 — API

* [ ] REST API
* [ ] WebSocket API
* [ ] Authentication
* [ ] Sessions
* [ ] Streaming
* [ ] OpenAPI

---

# Milestone 15 — CLI

* [ ] cortex init
* [ ] cortex run
* [ ] cortex serve
* [ ] cortex agent
* [ ] cortex tools
* [ ] cortex plugins
* [ ] cortex logs
* [ ] cortex replay

---

# Milestone 16 — Dashboard

* [ ] Login
* [ ] Agent view
* [ ] Timeline
* [ ] Event viewer
* [ ] Memory viewer
* [ ] Tool executions
* [ ] Cost dashboard
* [ ] Settings

---

# Milestone 17 — SDKs

Python

* [ ] Client SDK
* [ ] Tool decorators
* [ ] Agent SDK
* [ ] Async support

TypeScript

* [ ] Client SDK
* [ ] Tool SDK

---

# Milestone 18 — Testing

* [ ] Unit tests
* [ ] Integration tests
* [ ] E2E tests
* [ ] Performance tests
* [ ] Stress tests
* [ ] Fuzz tests

---

# Milestone 19 — Security

* [ ] Authentication
* [ ] Authorization
* [ ] Secret management
* [ ] Rate limiting
* [ ] Audit logs
* [ ] Encryption

---

# Milestone 20 — Release

* [ ] Versioning
* [ ] Changelog
* [ ] Release automation
* [ ] Documentation site
* [ ] Examples
* [ ] Benchmarks
* [ ] First public release

---

# Nice to Have

* [ ] Multi-agent orchestration
* [ ] Distributed workers
* [ ] Kubernetes operator
* [ ] Workflow editor
* [ ] Mobile app
* [ ] Marketplace
* [ ] Plugin registry
* [ ] Agent templates
* [ ] Hosted cloud version

---

# Technical Debt

* None currently.

---

# Bugs

Use this section for confirmed bugs only.

---

# Future Research

Move ideas here once they require investigation before implementation.

* [ ] Adaptive loop optimisation
* [ ] Self-healing runtimes
* [ ] Multi-model scheduling
* [ ] Automatic prompt optimisation
* [ ] Distributed checkpointing
* [ ] CRDT-based state replication

---

# Release Checklist

Before every release:

* [ ] Tests passing
* [ ] Lint passing
* [ ] Formatting clean
* [ ] Documentation updated
* [ ] Changelog updated
* [ ] Version bumped
* [ ] Benchmarks run
* [ ] Security review completed
* [ ] Release tagged
* [ ] Artifacts published
* [ ] Announcement prepared