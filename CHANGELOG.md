# Changelog

All notable changes to Cortex are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Install**: `scripts/install.sh` (curl → `~/.local/bin`), `cortex setup`,
  `cortex doctor`, user home `~/.cortex` (`CORTEX_HOME`), docs/install.md
- **Project instructions**: auto-load `.cortex/instructions.md` / `AGENTS.md` /
  `CLAUDE.md` (etc.) into agent context
- **`cortex init --web3`**: Foundry MCP sample + Web3 instruction scaffold
- **`cortex update`**: reinstall guidance (Unix; optional `CORTEX_UPDATE_EXEC=1`)
- **`cortex run --stream`**: stream assistant text deltas to stderr

### Changed

- **Docker CI**: cargo-chef + BuildKit cache mounts, tighter `.dockerignore`,
  PR Docker builds only when packaging files change (faster pipeline)
- **Dependencies**: axum 0.8, sqlx 0.8, thiserror 2, tokio-tungstenite 0.26,
  tower-http 0.6; Python httpx/pytest floors raised; assorted Actions bumps
- **TUI**: ratatui 0.30 (closes transitive `lru` advisory GHSA-rhfx-m35p-ff5j)
- **CI**: Dependabot groups + patch/minor auto-merge; majors stay human-reviewed
- **Config paths**: project `.cortex` → `~/.cortex` → monorepo `config/` →
  auto-bootstrap home; sessions DB falls back to home when no project dir

## [0.2.0] — 2026-07-09

Coding agent OS + smart-contract security arc (Phases 17–25).

### Added

- Full agent OS MVP (phases 0–16) and post-MVP follow-ups
- **CLI**: `run`, `chat`, `init`, `tools`, `models`, `sessions`, `workspace`,
  `skills`, `security`, `plugins`, `memory`, `parse`, `tui`, `serve`, `eval`
- **Providers**: OpenAI-compatible, Anthropic, mock (offline)
- **Tools**: filesystem, shell (optional bubblewrap), git, HTTP, docker,
  web search, apply_patch, browser CDP (Obscura/Chrome), memory search,
  code outline / workspace symbols / definition, skill evolution tools
- **Skills**: capability packs including `skill_creator` and `frontend_design`
  (adapted from Anthropic skills guidance)
- **Smart contract security**: builtin `sc_security` skill + hardened `solidity`
  prompts (ETHSkills-style checklist, audit report format, Foundry/Slither
  workflow); system identity as coding agent + SC security; project markers for
  remappings/Slither; MCP examples and docs for [skills.eth.sh](https://skills.eth.sh/)
  (Pashov, QuillShield, Foundry MCP, Blockscout, Tenderly)
- **Multi-lens audits (P17)**: `audit_lenses` tool runs parallel specialty
  sub-agents (access, reentrancy, economic, proxy, invariants) with shared
  Solidity source bundles under `.cortex/tmp/`
- **Solidity outlines (P18)**: tree-sitter Solidity for `code_outline` /
  workspace symbols (contracts, functions, modifiers, events, …)
- **Foundry samples (P19)**: `examples/foundry-vault/`, `examples/mcp/foundry.mcp.toml`,
  `scripts/smoke_foundry.sh`
- **MCP HTTP (P20)**: Streamable HTTP transport + legacy SSE fallback for remote
  servers (Blockscout, Tenderly, CoinGecko, …); header env expansion; local-host guard
- **SC tooling depth (P21)**: `sc_xray` pre-audit skill; `sc_analyzers` / `sc_poc` /
  `findings_schema` prompts; demo reentrancy PoC under `examples/foundry-vault`
- **Audit artifacts (P22)**: `write_audit_report` tool (markdown/JSON under
  `.cortex/audits/`); vuln fixtures + SC eval cases
- **Web3 skill bridge (P23)**: `cortex skills import` for SKILL.md; `web3_catalog`
  skill; recipes for skills.eth.sh MCP/packs; load `.cortex/prompts`
- **Loop quality (P24)**: parallel read-only tool batches; `--plan` mode;
  `--verify` / `--verify-cmd` after file writes
- **Productization (P25)**: honest roadmap, DECISIONS ADRs, README status for 0.2.0
- **Memory**: SQLite sessions/checkpoints, rolling summaries, local vector index
- **Plugins**: builtins + external `plugin.toml` directory plugins
- **HTTP API**: `/v1/*` including `POST /v1/runs` and SSE `/v1/runs/stream`
- **Python SDK**: sync/async HTTP client under `sdks/python`
- **Evals**: fixture suite under `evals/` (`cortex eval run`)
- **CI/CD**: GitHub Actions (lint, test, eval, smoke, deny, Python, release
  builds), Dependabot, Docker/GHCR workflow, `scripts/ci_local.sh`, Makefile

### Security

- Policy TOML, path sandbox, shell denylists, env scrub, HTTP host blocks
- Optional bubblewrap isolation for shell (`shell_use_bubblewrap`)
- Optional API bearer token (`CORTEX_API_TOKEN`)

## [0.1.0] — 2026-07-09

Initial early development snapshot (agent OS MVP through Phase 16 + early follow-ups).
