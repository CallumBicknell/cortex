//! Cortex plugins: in-process host for extending tools and lifecycle.
//!
//! v0.1 supports **builtin** plugins compiled into the binary (e.g. `echo`).
//! Dynamic loading (`cdylib` / external processes) is intentionally deferred.

#![deny(missing_docs)]

mod builtins;
mod config;
mod error;
mod host;
mod plugin;

pub use builtins::{builtin_ids, create_builtin};
pub use config::{PluginEntry, PluginsConfig};
pub use error::{PluginError, Result};
pub use host::{PluginHost, PluginState, PluginStatus};
pub use plugin::{Plugin, PluginContext, PluginMeta};
