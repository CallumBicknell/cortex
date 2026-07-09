//! Skills: capability packs (tools + prompts + tags), not hard-coded modes.
//!
//! The planner/runtime activates skills heuristically from the user prompt and
//! project fingerprint, or from an explicit allow-list.

#![deny(missing_docs)]

mod builtins;
mod registry;
mod select;
mod skill;

pub use builtins::builtin_skills;
pub use registry::SkillRegistry;
pub use select::{select_skills, SkillSelection};
pub use skill::Skill;
