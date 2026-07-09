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

* [x] P2.1 `cortex-llm` + `Provider` trait
* [x] P2.2 Chat request/response types
* [x] P2.3 OpenAI-compatible HTTP client
* [x] P2.4 Anthropic adapter
* [x] P2.5 Ollama / OpenRouter / LM Studio via openai-compatible
* [x] P2.6 Provider registry + `config/models.toml`
* [x] P2.7 Mock provider for tests
* [x] P2.8 Retries & timeouts
* [x] P2.9 Streaming MVP (OpenAI SSE + mock deltas)

---

# Phase 3 — Tools

* [x] P3.1 `Tool` trait + `ToolResult` integration
* [x] P3.2 `ToolRegistry`
* [x] P3.3 `ToolExecutor`
* [x] P3.4 `ToolContext`
* [x] P3.5 Permissions / path sandbox
* [x] P3.6 Filesystem tools
* [x] P3.7 Shell tool
* [x] P3.8 Git tools
* [x] P3.9 HTTP tool (SSRF-safe)
* [x] P3.10 JSON Schema for tool-calling (`Tool::spec`)
* [x] P3.11 Unit tests

---

# Phase 4 — Agent loop

* [x] P4.1 Loop state machine (`LoopPhase` transitions)
* [x] P4.2 `AgentLoop` in `cortex-runtime`
* [x] P4.3 Context builder MVP
* [x] P4.4 LLM → tool calls → observe → repeat
* [x] P4.5 Finish conditions (final answer, max turns, cancel)
* [x] P4.6 In-memory session (`Session` mutation)
* [x] P4.7 Tracing + optional event bus publish
* [x] P4.8 Integration test with mock provider
* [x] P4.9 Reflect stub
* [x] P4.10 Verify stub

---

# Phase 5 — CLI product

* [x] P5.1 `cortex-cli` binary (`cortex`)
* [x] P5.2 `cortex init`
* [x] P5.3 `cortex run`
* [x] P5.4 `cortex chat`
* [x] P5.5 `cortex tools list`
* [x] P5.6 `cortex models list`
* [x] P5.7 Flags (`--model`, `--workspace`, `--yolo`, `--max-turns`, `--json`)
* [x] P5.8 `.env` loading
* [x] P5.9 Examples (`examples/hello_agent.md`)
* [x] P5.10 Agent smoke script (`scripts/smoke_agent.sh`)

---

# Phase 6 — Persistence (SQLite)

* [x] P6.1 `cortex-memory` + SQLx SQLite
* [x] P6.2 Migrations (`migrations/001_init.sql`)
* [x] P6.3 Session store API (save/load/list/archive)
* [x] P6.4 Event log + tool trace
* [x] P6.5 Checkpoints + `persist_run`
* [x] P6.6 CLI `sessions list|show|resume|export|archive`
* [x] P6.7 Auto-save after `run` / `chat` (`.cortex/data/cortex.db`)
* [x] P6.8 Summaries table (schema ready)

---

# Phase 7 — Workspace + context engineering

* [x] P7.1 `cortex-workspace` (root detect, ignore, project, repo map)
* [x] P7.2 Ignore engine (`.gitignore` + `.cortexignore`)
* [x] P7.3 Project detect (languages, test/lint commands)
* [x] P7.4 Repo map MVP
* [x] P7.5 `cortex-context` budgeted builder
* [x] P7.6 Token estimate (chars/4)
* [x] P7.7 History compression
* [x] P7.8 CLI `workspace info|map` + inject map into run/chat

---

# Phase 8 — Prompts + skills

* [x] P8.1 `cortex-prompts` (markdown load + `{{var}}`)
* [x] P8.2 Core + skill prompts under `prompts/`
* [x] P8.3 `cortex-skills` registry + builtin packs
* [x] P8.4 Packs: coding/git/web/testing/rust/python/javascript/solidity/review
* [x] P8.5 Heuristic selection (prompt + project + `--skills`)
* [x] P8.6 Dynamic tool exposure via `ContextBuilder::allowed_tools`
* [x] P8.7 `docs/skills.md` + CLI `skills list|select`

---

# Phase 9 — Security

* [x] P9.1 `cortex-security` policy crate + `config/security.toml`
* [x] P9.2 PolicyApprover + interactive CLI + `--yolo`
* [x] P9.3 Tool modes, path sandbox, HTTP host blocks, shell deny patterns
* [x] P9.4 Shell env scrub + cwd sandbox
* [x] P9.5 Secrets redaction (text/JSON)
* [x] P9.6 Approval audit → SQLite `permissions_audit`
* [x] P9.7 Tests + `cortex security show` + `docs/security.md`

---

# Phase 10 — MCP + advanced tools

* [x] P10.1 MCP stdio client (initialize, tools/list, tools/call)
* [x] P10.2 `config/mcp.toml` + CLI bootstrap registration
* [x] P10.3 Browser via MCP (documented; not hard-coded)
* [x] P10.4 `docker_run` tool
* [x] P10.5 `web_search` (Tavily/Brave)
* [x] P10.6 `apply_patch` tool
* [x] P10.7 Mock MCP stdio integration test + `docs/mcp.md`

---

# Later phases (summary)

See implementation plan for full task lists:

| Phase | Theme |
|-------|--------|
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
