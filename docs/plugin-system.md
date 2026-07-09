# Plugin system

Cortex plugins extend the runtime **in-process**: they run through a lifecycle
(`init` → `start` → `stop`) and can register tools on the shared
`ToolRegistry`.

**Current scope:**

- **Builtin** plugins compiled into the binary (`echo`, …)
- **External** directory plugins (`plugin.toml` + shell/command tools),
  auto-discovered under `.cortex/plugins/` and `plugins/`

True `cdylib` loading is still deferred (unsafe ABI).

## Concepts

| Concept | Role |
|---------|------|
| **Skill** | Capability pack — which *existing* tools/prompts to expose for a task |
| **Plugin** | Code extension — *adds* tools (and later hooks) into the process |
| **MCP server** | External process tools over the Model Context Protocol |

Use a skill when you only need to select tools. Use a plugin when you need new
code. Use MCP when the capability already exists as an MCP server.

## Config

`config/plugins.toml`, `.cortex/plugins.toml`, or `CORTEX_PLUGINS_CONFIG`:

```toml
enabled = true

[[plugins]]
id = "echo"
enabled = true
# settings = { prefix = "P:" }
```

| Variable | Meaning |
|----------|---------|
| `CORTEX_PLUGINS_CONFIG` | Path to plugins.toml |

## Builtin plugins

| Id | Tools | Description |
|----|-------|-------------|
| `echo` | `plugin_echo` | Demo: echoes a message (optional `prefix` setting) |

## CLI

```bash
cargo run -p cortex-cli -- plugins list
cargo run -p cortex-cli -- tools list   # includes plugin_* tools when loaded
```

## Implementing a builtin plugin

1. Implement `cortex_plugins::Plugin` in `crates/cortex-plugins/src/builtins/`.
2. Register the id in `create_builtin` / `builtin_ids`.
3. In `init`, call `ctx.tools.register(...)` (or `register_or_replace`).
4. Enable the id in `plugins.toml`.
5. Add tests in `cortex-plugins`.

```rust
use async_trait::async_trait;
use cortex_plugins::{Plugin, PluginContext, PluginMeta, Result};

pub struct MyPlugin;

#[async_trait]
impl Plugin for MyPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("my_plugin", "My Plugin", "0.1.0", "does a thing")
    }

    async fn init(&mut self, ctx: &mut PluginContext<'_>) -> Result<()> {
        // ctx.tools.register(Arc::new(MyTool))?;
        let _ = ctx;
        Ok(())
    }
}
```

## Lifecycle

1. **Load** — factory creates `Box<dyn Plugin>` for each enabled id.
2. **Init** — `init` runs with `PluginContext` (workspace + tools + settings).
3. **Start** — all plugins `start()` after every `init` succeeded.
4. **Stop** — reverse order on clean shutdown (`PluginHost::stop`).

If any `init` fails, bootstrap fails (fail closed).

## Security

- Plugins run **in-process** with full agent privileges — only enable trusted ids.
- Prefer MCP or the sandbox shell for untrusted third-party code.
- Tool permission modes in `security.toml` still apply to plugin-contributed tools.

## Shipped external plugins

| Path | Tools | Notes |
|------|-------|-------|
| `plugins/example_echo/` | `ext_example_echo` | Demo |
| `plugins/foundry_helpers/` | `forge_version`, `forge_build`, `forge_test`, `forge_test_verbose`, `forge_test_match`, `forge_test_fuzz`, `forge_fmt_check` | Fixed-arg Foundry; needs `forge` on PATH; `cwd = "{workspace}"` |
| `plugins/sc_analyzers/` | `slither_version`, `slither_scan`, `slither_scan_path`, `slither_human_summary`, `aderyn_version`, `aderyn_scan` | Static analyzers; `allow_nonzero` so Slither findings still return output |

`cortex init --web3` copies both into `.cortex/plugins/`.

## External plugin layout

```text
plugins/my_tool/
  plugin.toml
  # optional scripts next to the manifest
```

```toml
id = "my_tool"
description = "Does a thing"
[[tools]]
name = "my_tool_run"
description = "Run the thing"
command = ["python3", "{workspace}/plugins/my_tool/run.py", "{arg:path}"]
[tools.parameters]
path = { type = "string" }
```

Placeholders: `{workspace}`, `{args_json}`, `{arg:KEY}`.

## Not yet implemented

- Dynamic `cdylib` loading
- Process isolation for external plugin commands beyond env scrub
- Plugin marketplace / install CLI
- Event-bus hooks (`on_before_tool`, etc.)
- Python plugin host

## Related

- [`docs/skills.md`](skills.md) — capability packs
- [`docs/mcp.md`](mcp.md) — external tool servers
- [`docs/browser.md`](browser.md) — CDP browser tools
