# Changelog

All notable changes to Cortex are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- **Mock setup**: loading `models.toml` no longer fails when Anthropic is listed
  without `ANTHROPIC_API_KEY` (provider/alias skipped until a key is set)
- **Chat TUI**: never write tracing to stderr in chat/tui/setup (logs go to
  `~/.cortex/logs/cortex.log`) so INFO lines cannot paint over the alternate
  screen; start a fresh session (resume via Ctrl+B)
- **TUI cursor**: `accept_completion` set cursor to byte length instead of char
  count, breaking position after tab-completion on non-ASCII input
- **TUI session list**: status column used Debug format (`{:?}`); now shows
  lowercase labels (active, completed, failed, …)
- **TUI Shift key**: some terminals send `KeyCode::Char('a')` with `SHIFT`
  instead of `KeyCode::Char('A')`; `apply_shift()` converts letters to
  uppercase and number-row to symbols

### Changed

- **`cortex chat`**: full-screen Claude Code–style TUI by default (use `--plain`
  for the old line REPL); cleaner conversation layout, multi-line composer,
  sessions modal, live stream + tool chips
- **TUI input**: cursor is now char-index based (safe for non-ASCII) with
  `char_to_byte` helper for all string operations

### Added

- **Chat TUI**: `/skill` slash commands and `@path` file/folder mentions with
  Tab autocomplete (↑/↓ select); attachments inlined for the agent; `/skills` list
- **Chat TUI**: tool errors show a short reason (e.g. CDP not running) instead of
  bare `[ERR] browser_navigate`; CDP connect message points at `obscura serve`
- **Browser tools**: auto-start local Obscura/Chrome when CDP is not listening
  (`auto_start = true` by default; loopback only)
- **Chat TUI**: enable bracketed paste so clipboard paste lands in the composer
  (including multi-line) instead of being ignored or firing accidental Enter
- **`plugins/sc_analyzers`**: fixed-arg `slither_*` / `aderyn_*` tools
  (`allow_nonzero` so findings still return output)
- **`cortex init --web3`** also installs `sc_analyzers` plugin
- External plugin flag `allow_nonzero` for analyzer-style CLIs
- **TUI**: live token streaming, tool/sub-agent log events, run summary
  (turns · tools ok/err · duration)
- **`cortex setup` TUI wizard**: full-screen provider picker with auto-detect
  (OpenAI / Anthropic / OpenRouter keys, Ollama :11434), custom OpenAI-compatible
  providers, Anthropic native; `--default-model` / `--no-wizard` for scripts
- **Local install**: `make install` / `scripts/install-local.sh` → `~/.local/bin/cortex`
- **TUI readline shortcuts**: Ctrl+A (home), Ctrl+E (end), Ctrl+W (delete word),
  Ctrl+U (delete to start), Ctrl+K (delete to end)
- **TUI word movement**: Ctrl+Left/Right moves cursor by word
- **TUI streaming cursor**: blinking block cursor (▌) during assistant stream
- **TUI auto-scroll**: conversation follows new streaming content; scroll-up
  pauses auto-follow with "↓ new content below" indicator
- **TUI /undo**: removes last user+assistant exchange and restores the prompt
- **TUI tool elapsed time**: shows `(X.Xs)` next to active tool chip
- **TUI adaptive footer**: shorter hints on narrow terminals (<60 / <80 cols)
- **TUI message truncation**: streaming and history truncation use
  `floor_char_boundary` to avoid splitting multi-byte characters
- **TUI tool-approval modal**: non-yolo users can approve/deny tool calls
  via Y/N keys (oneshot channel to `TuiApprover`)
- **TUI keep partial reply on cancel**: Ctrl+C preserves draft assistant
  text instead of discarding it
- **TUI session search + archive**: type to filter in session drawer,
  'd' to archive a session
- **TUI composer undo**: Ctrl+Z restores previous input state (100-entry stack)
- **TUI `/compact`**: toggle compact mode (hide header row)
- **TUI token usage**: footer shows ↑prompt ↓completion token counts
- **TUI Shift+Enter**: inserts newline in composer (Kitty keyboard protocol
  with fallback for unsupported terminals)
- **TUI Ctrl+Up/Down**: scrolls conversation history (3 lines)
- **TUI Ctrl+O**: copies last assistant reply to OS clipboard
  (pbcopy/xclip/xsel/wl-copy)
- **TUI `/stats`**: shows session id, message counts, content chars,
  estimated tokens, and token usage for the session
- **TUI `/rename <name>`**: renames session display label (shown in
  header instead of database path)
- **TUI Ctrl+Home/End**: jump to top/bottom of conversation
- **TUI soft-wrap composer**: long lines grow the composer box instead
  of clipping; caret scrolls into view when content exceeds box height
- **TUI composer char count**: title bar shows live chars/lines count
- **TUI elapsed turn timer**: footer shows running time during agent turns
  (e.g. '12s' or '02:15')
- **TUI streaming char count**: footer shows live character count while
  assistant is generating (e.g. '342 chars')
- **TUI scroll indicator**: shows message count below when scrolled up
  (e.g. '↓ 5 messages below')
- **TUI session auto-save**: sessions auto-save to SQLite on user message,
  cancel, and run completion — no more lost conversations on crash
- **TUI tool output visibility**: sub-agent logs (↳ prefix) now show in
  conversation view for better observability
- **TUI error display**: errors show capitalized 'Error: {msg}' in
  conversation; failed runs without error show explicit failure message
- **TUI session label in footer**: renamed sessions show label in footer
  as '[label]' for persistent context awareness
- **TUI archive confirmation**: archive shows 'archived session {short_id}'
  in status bar instead of generic message

## [0.2.1] — 2026-07-09

Daily-driver install path + Wave A polish + Foundry helpers.

### Added

- **Install**: `scripts/install.sh` (curl → `~/.local/bin`), `cortex setup`,
  `cortex doctor`, user home `~/.cortex` (`CORTEX_HOME`), docs/install.md
- **Project instructions**: auto-load `.cortex/instructions.md` / `AGENTS.md` /
  `CLAUDE.md` (etc.) into agent context
- **`cortex init --web3`**: Foundry MCP sample + Web3 instructions +
  `foundry_helpers` plugin (`forge_build` / `forge_test` / …)
- **`cortex update`**: reinstall guidance (Unix; optional `CORTEX_UPDATE_EXEC=1`)
- **`cortex run --stream`**: stream assistant text deltas to stderr
- **`plugins/foundry_helpers`**: fixed-arg forge tools; plugin `cwd = "{workspace}"`

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
