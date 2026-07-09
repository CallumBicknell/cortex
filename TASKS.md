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
* [x] P10.8 Native CDP browser tools (Obscura / Chrome / custom) + `docs/browser.md`

---

# Phase 11 — Plugin system (in-process MVP)

* [x] P11.1 `cortex-plugins` crate: `Plugin` trait, `PluginMeta`, `PluginContext`
* [x] P11.2 `PluginsConfig` + `config/plugins.toml`
* [x] P11.3 `PluginHost` lifecycle (load/init/start/stop)
* [x] P11.4 Builtin `echo` plugin → `plugin_echo` tool
* [x] P11.5 CLI bootstrap load + `cortex plugins list`
* [x] P11.6 Tests + honest `docs/plugin-system.md`

---

# Phase 12 — Summaries + embeddings

* [x] P12.1 Rolling conversation summarizer (LLM + extractive fallback)
* [x] P12.2 ContextBuilder: `rolling_summary` + `retrieval` sections
* [x] P12.3 Agent loop auto-summarize + SQLite `summaries` load/save
* [x] P12.4 Local embedder + cosine search + `embeddings` migration
* [x] P12.5 `VectorStore` upsert/search + workspace index
* [x] P12.6 `memory_search` tool + `memory` skill
* [x] P12.7 CLI `cortex memory {index,search,stats,summarize}`
* [x] P12.8 Mock provider embeddings + tests + `docs/memory.md`

---

# Phase 13 — Parser / tree-sitter (outline MVP)

* [x] P13.1 `cortex-parse` crate with tree-sitter Rust + Python
* [x] P13.2 Symbol outline extraction (walk-based)
* [x] P13.3 `code_outline` tool + coding skill
* [x] P13.4 CLI `cortex parse outline`
* [x] P13.5 Tests + `docs/parse.md`
* [ ] P13.6 Full LSP (deferred)

---

# Phase 14 — TUI

* [x] P14.1 `cortex-tui` crate (ratatui + crossterm)
* [x] P14.2 Layout: sessions / transcript / tool log / input
* [x] P14.3 Keyboard UX (send, cancel, yolo, reload, new session)
* [x] P14.4 Wire agent turns via `TuiHost` + `AgentLoop`
* [x] P14.5 CLI `cortex tui` + `docs/tui.md`

---

# Phase 15 — HTTP API + Python SDK

* [x] P15.1 `cortex-api` axum router (health, info, models, tools, sessions, runs)
* [x] P15.2 Optional bearer token auth
* [x] P15.3 CLI `cortex serve`
* [x] P15.4 Integration tests (health, auth, mock run)
* [x] P15.5 Python `CortexClient` / `AsyncCortexClient` against HTTP API
* [x] P15.6 Python unit tests + `docs/api.md`

---

# Phase 16 — Hardening, sub-agents, evals

* [x] P16.1 Run budgets (`max_run_secs`, `max_tool_calls_per_turn`)
* [x] P16.2 Security harden helpers (path escape, bubblewrap detect)
* [x] P16.3 Sub-agent runner + depth limits
* [x] P16.4 `spawn_subagent` tool + `tools_with_subagent`
* [x] P16.5 `cortex-eval` fixture harness + `evals/*.toml`
* [x] P16.6 CLI `cortex eval list|run` + `docs/hardening.md`

---

# Plan complete (M0–M16 MVP) + follow-ups

## Follow-ups

* [x] F1 External directory plugins (`plugin.toml` + auto-discover)
* [x] F2 Bubblewrap-wired shell (`shell_use_bubblewrap`)
* [x] F3 Streaming API `POST /v1/runs/stream` (SSE)
* [x] F4 Sub-agent lifecycle events on parent bus
* [x] F5 Workspace symbols + code_definition (LSP-lite)
* [x] F6 Self-evolving skills (skill_list/save/promote + disk store)
* [x] F7 Coding + SC security identity (`sc_security`, eth.sh docs)

---

# Phase 17 — Multi-lens audit orchestration

* [x] P17.1 Specialty lens prompts (`prompts/skills/audit_lenses/*`)
* [x] P17.2 `audit_lenses` tool (parallel JoinSet + semaphore)
* [x] P17.3 Shared source bundle (`.cortex/tmp/audit-*/source.md`)
* [x] P17.4 Dedup / orchestrator footer in tool output
* [x] P17.5 Wire into `sc_security` skill + permissions
* [x] P17.6 Budgets (max 5 lenses, default 4, turns clamp)
* [x] P17.7 Unit + mock parallel tests
* [x] P17.8 `docs/web3-security.md` multi-lens section

---

# Phase 18 — Solidity code intelligence

* [x] P18.1 `tree-sitter-solidity` workspace dep
* [x] P18.2 `SourceLanguage::Solidity`
* [x] P18.3 Outline walk (contract/fn/modifier/event/…)
* [x] P18.4 Index via existing workspace walk (`.sol` extension)
* [x] P18.5 Unit tests on Vault fixture
* [x] P18.6 `docs/parse.md` language table

---

# Phase 19 — Foundry MCP sample + smoke

* [x] P19.1 `examples/mcp/foundry.mcp.toml`
* [x] P19.2 Demo fixture `examples/foundry-vault/`
* [x] P19.3 `scripts/smoke_foundry.sh` (skip-friendly)
* [x] P19.4 Docs: first Foundry session in `web3-security.md` / mcp.md

Optional later: `cortex init --web3`, CI foundry-smoke job.

---

# Phase 20 — MCP HTTP / Streamable HTTP

* [x] P20.1 Streamable HTTP transport (POST JSON / SSE response)
* [x] P20.2 Legacy SSE endpoint discovery fallback
* [x] P20.3 Config: `transport`, `url`, `headers`, `timeout_secs`, env expansion
* [x] P20.4 Local-host SSRF guard (`CORTEX_MCP_ALLOW_LOCAL`)
* [x] P20.5 Integration test (axum mock server)
* [x] P20.6 Docs + `config/mcp.toml` remote examples

---

# Phase 21 — SC security tooling depth

* [x] P21.1 Analyzer conventions prompt (`sc_analyzers`)
* [x] P21.2 `sc_xray` skill + pre-audit report prompt
* [x] P21.3 PoC scaffold prompt + demo exploit test sketch
* [x] P21.4 Shared findings schema (markdown + JSON)
* [x] P21.5 Wire into `sc_security` prompts; selection tests; docs

Optional later: `plugins/foundry_helpers` fixed-arg forge wrappers.

---

# Phase 22 — Audit artifacts + evals

* [x] P22.1 `write_audit_report` tool → `.cortex/audits/`
* [x] P22.2 Meta sidecar with session_id + fingerprint
* [x] P22.3 Optional findings JSON + memory index tip
* [x] P22.4 Eval fixtures (`evals/fixtures/vulns/*`) + sc_* eval TOMLs
* [x] P22.5 Wire into sc_security / sc_xray; docs

---

# Phase 23 — Web3 skill import / skills.eth.sh bridge

* [x] P23.1 `web3_catalog` skill + prompt
* [x] P23.2 `cortex skills import` (path/https SKILL.md → learned pack)
* [x] P23.3 Load `.cortex/prompts` into agent context
* [x] P23.4 Recipes doc (`docs/web3-recipes.md`)
* [x] P23.5 Tests + list learned skills in CLI

---

# Phase 24 — Agent loop quality

* [x] P24.1 Safe parallel tool batches (`execute_all` + `is_parallel_safe`)
* [x] P24.2 `--plan` mode injects plan guidance
* [x] P24.3 `--verify` / `--verify-cmd` after file mutations
* [x] P24.4 Docs (`docs/loop-quality.md`)

Deferred: LLM skill-pick step; richer TUI sub-agent UX; full token streaming polish.

---

# Phase 25 — Docs / release productization

* [x] P25.1 Honest `docs/roadmap.md` (shipped vs deferred)
* [x] P25.2 README status for coding + SC security (v0.2.0)
* [x] P25.3 Workspace version **0.2.0** + CHANGELOG section
* [x] P25.4 `DECISIONS.md` ADRs (multi-lens, Solidity parse, MCP HTTP, import, plan/verify)
* [x] P25.5 TASKS checklist closed for Phases 17–25

---

# Arc complete (Phases 17–25)

Post-MVP coding agent + smart-contract security wave is **done**.

# Phase 26 — Wave A daily driver (partial)

* [x] P26.1 User home `~/.cortex` + install.sh + setup/doctor
* [x] P26.2 Project instructions auto-load (AGENTS.md / …)
* [x] P26.3 `cortex init --web3`
* [x] P26.4 `cortex update`
* [x] P26.5 CLI `--stream` (text deltas via event bus)
* [ ] P26.6 GitHub Release tag so install.sh has assets
* [ ] P26.7 TUI stream + sub-agent summary
* [ ] P26.8 Interactive setup wizard

Still open (later):

| Theme | Notes |
|-------|--------|
| Full LSP | Diagnostics, hover, true go-to via language servers |
| Dynamic cdylib | Unsafe plugin ABI |
| Firejail/seccomp | Stronger OS isolation |
| LLM skill-pick | When tag heuristics are ambiguous |
| Foundry helpers plugin | Fixed-arg forge tools |
| Foundry helper plugin | Fixed-arg forge wrappers |

See [`docs/roadmap.md`](docs/roadmap.md).

---

# Archived

The previous multi-thousand-line EventBus `history_*` backlog was removed as
premature. Do not resurrect it until a real product loop needs those features.
