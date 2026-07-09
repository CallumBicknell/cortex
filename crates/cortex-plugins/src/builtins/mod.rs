//! Builtin in-process plugins shipped with Cortex.

mod echo;

use crate::plugin::Plugin;
use echo::EchoPlugin;

/// Create a builtin plugin by id, if known.
pub fn create_builtin(id: &str) -> Option<Box<dyn Plugin>> {
    match id {
        "echo" => Some(Box::new(EchoPlugin::new())),
        _ => None,
    }
}

/// Ids of all builtin plugins.
pub fn builtin_ids() -> &'static [&'static str] {
    &["echo"]
}
