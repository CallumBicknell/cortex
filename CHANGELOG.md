# Changelog

All notable changes to Cortex are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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

Initial early development snapshot corresponding to the Unreleased feature set
above (first public-shape MVP on the feature branch).
