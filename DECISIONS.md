# Architecture Decision Records

Lightweight ADRs for Cortex. Newest first.

---

## ADR-005 — Plan mode and verify-after-write as loop flags (2026-07)

**Status:** Accepted
**Context:** Users want Claude Code–like “plan first” and automatic test runs after edits without hard-coding a global mode.
**Decision:**
- `--plan` injects plan-mode system guidance (read/plan before large writes).
- `--verify` / `--verify-cmd` run one `shell` tool after successful file mutations.
- Safe parallel tool batches for read-only tools; writes/shell/agents stay serial.

**Consequences:** Clear CLI UX; verify can be expensive on monorepos — prefer targeted `--verify-cmd`.

---

## ADR-004 — SKILL.md import without vendoring eth.sh packs (2026-07)

**Status:** Accepted
**Context:** [skills.eth.sh](https://skills.eth.sh/) hosts many third-party skills; vendoring them would bloat the monorepo and create license/update churn.
**Decision:**
- Builtin `web3_catalog` documents discovery.
- `cortex skills import` (path or https) converts SKILL.md → `.cortex/skills/*.toml` + `.cortex/prompts/skills/*.md`.
- No automatic download on agent start.

**Consequences:** Users opt in per pack; prompt bodies load via `PromptCatalog::load_dir(.cortex/prompts)`.

---

## ADR-003 — Streamable HTTP for remote MCP (2026-07)

**Status:** Accepted
**Context:** skills.eth.sh remote servers (Blockscout, Tenderly, CoinGecko) need HTTP, not only stdio. MCP 2025-03-26 defines Streamable HTTP; older HTTP+SSE used an `endpoint` event.
**Decision:**
- Primary: Streamable HTTP (`POST` JSON, accept JSON or SSE).
- Fallback: legacy SSE GET for `endpoint` event, then POST messages.
- Config: `transport = "http" | "sse" | "stdio"`, headers with `$ENV` expansion.
- Block localhost unless `CORTEX_MCP_ALLOW_LOCAL=1`.

**Consequences:** Unlocks remote Web3 MCP; protocol fragmentation handled by fallback path.

---

## ADR-002 — Tree-sitter Solidity for outlines (2026-07)

**Status:** Accepted
**Context:** Audits and coding need symbol maps for `.sol`; full LSP is out of scope.
**Decision:** Add `tree-sitter-solidity` and walk-based outlines (contracts, functions, modifiers, events, …) parallel to Rust/Python.
**Consequences:** `code_outline` / workspace symbols work on Solidity; not a full language server.

---

## ADR-001 — Multi-lens audits as a native tool, not vendored Pashov (2026-07)

**Status:** Accepted
**Context:** Pashov-style multi-agent audits are valuable but the full Claude skill tree (12 agents, remote VERSION checks) is runtime-specific and heavy.
**Decision:** Ship Cortex-native `audit_lenses`: 4–5 specialty prompts, shared source bundle, parallel sub-agents via `JoinSet`, orchestrator dedup footer. Link external packs (Pashov, QuillShield) via docs/import.
**Consequences:** Strong default audit loop without licensing or vendoring third-party skill repos.

---

## ADR-000 — Skills are capability packs, not hard-coded modes (2026)

**Status:** Accepted
**Context:** Product vision forbids “Solidity mode” as a global agent switch.
**Decision:** Skills declare tools + prompts + tags; selection is heuristic (prompt + project) or `--skills`. Always-on `coding` provides baseline.
**Consequences:** Solidity, sc_security, sc_xray, etc. compose without forking the agent OS.
