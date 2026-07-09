//! Cortex plugins: in-process host for extending tools and lifecycle.
//!
//! - **Builtin** plugins compiled into the binary (e.g. `echo`)
//! - **External** directory plugins (`plugin.toml` + command tools), auto-discovered
//!   under `.cortex/plugins/` and `plugins/`
//!
//! True `cdylib` loading remains out of scope (unsafe ABI surface).

#![deny(missing_docs)]

mod builtins;
mod config;
mod error;
mod external;
mod host;
mod plugin;

pub use builtins::{builtin_ids, create_builtin};
pub use config::{PluginEntry, PluginsConfig};
pub use error::{PluginError, Result};
pub use external::{
    discover_plugin_dirs, load_manifest, ExternalManifest, ExternalPlugin, ExternalToolDef,
};
pub use host::{PluginHost, PluginState, PluginStatus};
pub use plugin::{Plugin, PluginContext, PluginMeta};
