# Install Cortex

Cortex is a single `cortex` binary plus a user home directory (`~/.cortex`).

**Supported install targets:** Linux (`x86_64`) and macOS Apple Silicon (`aarch64`).
No Windows installer; Intel Mac release assets are not published (build from source).

## Quick install (Linux / macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/CallumBicknell/cortex/main/scripts/install.sh | sh
```

This downloads the latest [GitHub Release](https://github.com/CallumBicknell/cortex/releases), installs to `~/.local/bin/cortex`, and runs `cortex setup`.

| Env | Meaning |
|-----|---------|
| `CORTEX_VERSION` | Pin a tag (`v0.2.0` or `0.2.0`) |
| `CORTEX_INSTALL_DIR` | Install dir (default `~/.local/bin`) |
| `CORTEX_REPO` | `owner/name` override |

```bash
CORTEX_VERSION=v0.2.0 sh scripts/install.sh
cortex doctor
```

## From source (dev machine — recommended while hacking)

After each feature merge, reinstall so `cortex` is available **from any directory**:

```bash
# From a clone of this repo:
make install          # release build → ~/.local/bin/cortex
# or: just install
# or: ./scripts/install-local.sh

# Faster iteration (debug binary):
make install-debug
```

Requires `~/.local/bin` on your `PATH` (already common on Linux). Override with:

```bash
CORTEX_INSTALL_DIR=/usr/local/bin make install
```

Also works:

```bash
cargo install --path crates/cortex-cli --locked --force
# installs to ~/.cargo/bin
```

From Git only (no clone):

```bash
cargo install --git https://github.com/CallumBicknell/cortex --locked --bin cortex
cortex setup
```

## Home vs project

| Path | Role |
|------|------|
| `~/.cortex/` (or `$CORTEX_HOME`) | **User global** — models, optional MCP/security, global skills/prompts, fallback sessions DB |
| `<project>/.cortex/` | **Project** — overrides, audits, local sessions after `cortex init` |

**Config precedence:** CLI flags / env → project `.cortex/<file>` → `~/.cortex/<file>` → monorepo `config/` (dev) → auto-bootstrap home.

**Database:** `CORTEX_DATABASE` → project `.cortex/data/cortex.db` if project dir exists → else `~/.cortex/data/cortex.db`.

### Home layout

```text
~/.cortex/
  models.toml
  .env              # optional secrets (chmod 600)
  .env.example
  skills/ prompts/ plugins/
  data/cortex.db
  cache/ logs/
```

### Commands

```bash
make install          # (from repo) rebuild + install to ~/.local/bin
cortex setup          # TUI wizard on TTY (auto-detect OpenAI/Anthropic/Ollama)
cortex setup --wizard # force TUI wizard
cortex setup --no-wizard
cortex setup --default-model ollama --ollama-model llama3.2
cortex setup --default-model anthropic
cortex doctor
cortex init           # create project .cortex/ (optional)
cortex init --web3    # + Foundry MCP + analyzer plugins
cortex update         # print reinstall command (Unix)
cortex update --dry-run
```

### Setup wizard (TUI)

On an interactive terminal, `cortex setup` opens a **ratatui** wizard:

| Option | Notes |
|--------|--------|
| **Auto** | Picks OpenAI → Anthropic → OpenRouter → Ollama → Mock from env / probe |
| Mock | Offline default |
| Ollama | Local; probes `127.0.0.1:11434` |
| OpenAI | Detects `OPENAI_API_KEY` |
| Anthropic | Detects `ANTHROPIC_API_KEY`; enables native `anthropic` provider |
| OpenRouter | Detects `OPENROUTER_API_KEY` |
| **Custom** | Any OpenAI-compatible `base_url` + model + `api_key_env` (Groq, vLLM, …) |

Keys: `↑/↓` select · `Enter` · `Tab` fields (custom) · `Esc` cancel · `s` skip to mock.

Secrets are **never** written to disk — only env var **names**.

### Project instructions

On each run, Cortex injects the first existing file (in order):

1. `.cortex/instructions.md`
2. `AGENTS.md`
3. `CLAUDE.md`
4. `.cursorrules`
5. `CORTEX.md`

Use this for monorepo rules (same idea as other coding agents).

## First run

```bash
export OPENAI_API_KEY=…    # or configure ollama in models.toml
cortex models list
cortex run "hello" --json --yolo
```

Default `default_model` is the offline **mock** provider until you change it.

## Uninstall

```bash
rm -f ~/.local/bin/cortex
rm -rf ~/.cortex            # removes config + local session data
```

## See also

- [`README.md`](../README.md) — product overview
- [`docs/ci.md`](ci.md) — release assets consumed by `install.sh`
- [`config/models.toml`](../config/models.toml) — embedded default providers
