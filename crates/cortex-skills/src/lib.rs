//! Skills: capability packs (tools + prompts + tags), not hard-coded modes.
//!
//! The planner/runtime activates skills heuristically from the user prompt and
//! project fingerprint, or from an explicit allow-list.
//!
//! Learned skills can be stored under `.cortex/skills/` and evolve over time.

#![deny(missing_docs)]

mod builtins;
mod error;
mod registry;
mod select;
mod skill;
mod store;

pub use builtins::builtin_skills;
pub use error::{Result as SkillResult, SkillError};
pub use registry::SkillRegistry;
pub use select::{select_skills, SkillSelection};
pub use skill::Skill;
pub use store::{propose_skill, SkillDocument, SkillOrigin, SkillStore};
