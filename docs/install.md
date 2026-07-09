# Install Cortex

Cortex is a single `cortex` binary plus a user home directory (`~/.cortex`).

**Supported install targets:** Linux and macOS (Unix). There is **no Windows installer** for now — build from source if you need a Windows binary later.

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

## From source

```bash
cargo install --git https://github.com/CallumBicknell/cortex --locked --bin cortex
cortex setup
```

Or from a clone:

```bash
cargo build --release -p cortex-cli
# binary: target/release/cortex
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
cortex setup          # create/update ~/.cortex
cortex setup --force  # rewrite home models.toml
cortex doctor         # paths + env key presence (never prints secrets)
cortex init           # create project .cortex/ (optional)
```

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
