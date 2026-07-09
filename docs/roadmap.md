# Cortex roadmap (honest)

This document tracks **what is shipped** versus **what is deferred**.
It replaces earlier aspirational quarterly marketing text.

## Shipped (through Phase 25)

### Agent OS MVP (Phases 0–16 + follow-ups)

- Kernel, event bus, domain models, LLM providers, tools, agent loop
- CLI / TUI / HTTP API + SSE / Python SDK
- SQLite sessions, summaries, vector memory
- Skills, plugins (builtin + directory), MCP stdio
- Security policy, bubblewrap shell, evals, CI/CD

### Coding + smart-contract security arc (Phases 17–25)

| Phase | Deliverable |
|-------|-------------|
| 17 | `audit_lenses` multi-lens parallel audits |
| 18 | Tree-sitter Solidity outlines |
| 19 | Foundry vault demo + MCP sample + smoke |
| 20 | MCP Streamable HTTP + legacy SSE fallback |
| 21 | `sc_xray`, analyzers/PoC prompts, findings schema |
| 22 | `write_audit_report` + SC eval fixtures |
| 23 | `skills import` + `web3_catalog` + eth.sh recipes |
| 24 | Parallel read tools, `--plan`, `--verify` |
| 25 | Docs/ADR polish, version **0.2.0** |

Primary docs: [`web3-security.md`](web3-security.md), [`web3-recipes.md`](web3-recipes.md),
[`loop-quality.md`](loop-quality.md), [`mcp.md`](mcp.md), [`skills.md`](skills.md).

## Next — Wave A “daily driver” (in progress / partial)

| Item | Status |
|------|--------|
| Unix install + `~/.cortex` | Done |
| Project instructions (`AGENTS.md` / `.cortex/instructions.md`) | Done |
| `cortex init --web3` | Done |
| `cortex update` | Done |
| CLI `--stream` token deltas | Done (TUI still basic) |
| Tag release so `install.sh` has assets | Ops (tag `v*`) |
| TUI stream + run/tool summary | Done |
| First-run setup wizard | Open |

## Later (not scheduled as hard commitments)

- Full LSP (diagnostics/hover) beyond tree-sitter outlines
- Dynamic cdylib plugins
- LLM-based skill selection when tag scores are ambiguous
- Optional `plugins/foundry_helpers` fixed-arg forge wrappers
- Slither/Aderyn structured wrappers
- Postgres multi-tenant backend
- Professional-audit productization (out of scope for open-source core)

## Versioning

| Version | Meaning |
|---------|---------|
| 0.1.0 | Initial MVP snapshot |
| **0.2.0** | SC security arc + loop quality |
| **0.2.1** | Install/home + Wave A + foundry_helpers plugin |
| 0.3.x | Loop intelligence / deeper SC tooling (TBD) |

See [`CHANGELOG.md`](../CHANGELOG.md) and [`DECISIONS.md`](../DECISIONS.md).
