# TASKS.md

> Active development backlog for Cortex.
>
> Source of truth for sequencing: the approved Agent OS implementation plan
> (Phase 0–16). This file tracks checkable work items for the current and
> upcoming phases only.
>
> Rules:
>
> * One logical task = one branch / one or more related commits
> * Check items off instead of deleting them
> * Do **not** re-introduce the inflated Epic 2 “history_*” method list
> * Prefer vertical slices over speculative APIs

---

# Legend

* [ ] Not started
* [x] Complete
* [~] In progress
* [!] Blocked

---

# Phase 0 — Stabilize & clean foundation

* [x] P0.1 Restore compiling kernel (rewrite from corrupted lib.rs)
* [x] P0.2 Module-split `cortex-core` (`config`, `lifecycle`, `event`, `bus`, `kernel`, `service_registry`)
* [x] P0.3 Remove `stub.py` and dead backup/tmp files
* [x] P0.4 Real in-memory event bus (subscribe/publish/history/unsubscribe)
* [x] P0.5 `EventEnvelope` model (`id`, `timestamp`, `kind`, `correlation_id`, `payload`)
* [x] P0.6 Kernel lifecycle + bus unit tests
* [x] P0.7 Rewrite this `TASKS.md` to match the plan
* [x] P0.8 Honest README (implemented vs planned)
* [x] P0.9 Config foundation (TOML + env + validate + `config/default.toml`)

**Exit criteria:** `cargo test` / `cargo clippy` green; no stub injectors.

---

# Phase 1 — Domain models + event system

* [x] P1.1 Create `cortex-common` (errors, ids)
* [x] P1.2 Create `cortex-models` (Message, ToolCall, Session, …)
* [x] P1.3 Agent events in `cortex-events` (user/assistant/tool/phase/checkpoint)
* [x] P1.4 Event bus hardening (`replay_since` + panic isolation)
* [x] P1.5 Cancellation tokens wired into kernel stop
* [x] P1.6 Serde golden JSON fixtures

---

# Phase 2 — Provider abstraction

* [ ] P2.1 `cortex-llm` + `Provider` trait
* [ ] P2.2 Chat request/response types
* [ ] P2.3 OpenAI-compatible HTTP client
* [ ] P2.4 Anthropic adapter
* [ ] P2.5 Ollama via openai-compatible
* [ ] P2.6 Provider registry + `config/models.toml`
* [ ] P2.7 Mock provider for tests
* [ ] P2.8 Retries & timeouts
* [ ] P2.9 Streaming MVP

---

# Phase 3 — Tools

* [ ] P3.1 `Tool` trait + `ToolResult`
* [ ] P3.2 `ToolRegistry`
* [ ] P3.3 `ToolExecutor`
* [ ] P3.4 `ToolContext`
* [ ] P3.5 Permissions / path sandbox
* [ ] P3.6 Filesystem tools
* [ ] P3.7 Shell tool
* [ ] P3.8 Git tools
* [ ] P3.9 HTTP tool (SSRF-safe)
* [ ] P3.10 JSON Schema for tool-calling
* [ ] P3.11 Unit tests

---

# Phase 4 — Agent loop

* [ ] P4.1 Loop state machine
* [ ] P4.2 `AgentLoop` in `cortex-runtime`
* [ ] P4.3 Context builder MVP
* [ ] P4.4 LLM → tool calls → observe → repeat
* [ ] P4.5 Finish conditions
* [ ] P4.6 In-memory session
* [ ] P4.7 Tracing spans
* [ ] P4.8 Integration test with mock provider
* [ ] P4.9 Reflect stub
* [ ] P4.10 Verify stub

---

# Phase 5 — CLI product

* [ ] P5.1 `cortex-cli` binary
* [ ] P5.2 `cortex init`
* [ ] P5.3 `cortex run`
* [ ] P5.4 `cortex chat`
* [ ] P5.5 `cortex tools list`
* [ ] P5.6 `cortex models list`
* [ ] P5.7 Flags (`--model`, `--workspace`, `--approve`, …)
* [ ] P5.8 `.env` loading
* [ ] P5.9 Examples
* [ ] P5.10 Agent smoke script

---

# Later phases (summary)

See implementation plan for full task lists:

| Phase | Theme |
|-------|--------|
| 6 | SQLite sessions, checkpoints, migrations |
| 7 | Workspace + context budgets |
| 8 | Prompts + skills (coding/git/web/solidity/…) |
| 9 | Security, approvals, sandbox |
| 10 | MCP + advanced tools |
| 11 | Plugin system |
| 12 | Summaries + embeddings |
| 13 | Parser / tree-sitter / LSP |
| 14 | TUI |
| 15 | HTTP API + real Python SDK |
| 16 | Hardening, sub-agents, evals |

---

# Archived

The previous multi-thousand-line EventBus `history_*` backlog was removed as
premature. Do not resurrect it until a real product loop needs those features.
