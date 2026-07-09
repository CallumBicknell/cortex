//! Skills: capability packs (tools + prompts + tags), not hard-coded modes.
//!
//! The planner/runtime activates skills heuristically from the user prompt and
//! project fingerprint, or from an explicit allow-list.
//!
//! Learned skills can be stored under `.cortex/skills/` and evolve over time.

#![deny(missing_docs)]

mod builtins;
mod error;
mod import;
mod registry;
mod select;
mod skill;
mod store;

pub use builtins::builtin_skills;
pub use error::{Result as SkillResult, SkillError};
pub use import::{
    import_from_markdown, normalize_skill_id, parse_skill_md, read_skill_source,
    write_imported_skill, ImportOptions, ImportedSkill, DEFAULT_IMPORT_TOOLS,
};
pub use registry::SkillRegistry;
pub use select::{select_skills, SkillSelection};
pub use skill::Skill;
pub use store::{propose_skill, SkillDocument, SkillOrigin, SkillStore};
