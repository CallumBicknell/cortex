//! Skill registry.

use crate::builtins::builtin_skills;
use crate::skill::Skill;
use std::collections::HashMap;

/// Registry of available skills.
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builtin skill packs.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        for skill in builtin_skills() {
            reg.register(skill);
        }
        reg
    }

    /// Builtins plus learned skills from a store (learned overrides same id).
    pub fn with_builtins_and_store(store: &crate::store::SkillStore) -> Self {
        let mut reg = Self::with_builtins();
        if let Ok(docs) = store.load_all() {
            for doc in docs {
                reg.register(doc.skill);
            }
        }
        reg
    }

    /// Register a skill (replaces same id).
    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.id.clone(), skill);
    }

    /// Get by id.
    pub fn get(&self, id: &str) -> Option<&Skill> {
        self.skills.get(id)
    }

    /// All skills sorted by id.
    pub fn all(&self) -> Vec<Skill> {
        let mut v: Vec<_> = self.skills.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// Always-on skills.
    pub fn always_on(&self) -> Vec<Skill> {
        self.all().into_iter().filter(|s| s.always_on).collect()
    }

    /// Skill ids.
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = self.skills.keys().cloned().collect();
        ids.sort();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_present() {
        let reg = SkillRegistry::with_builtins();
        assert!(reg.get("coding").unwrap().always_on);
        assert!(reg.get("solidity").is_some());
        assert!(reg.get("sc_security").is_some());
        assert!(reg.get("sc_xray").is_some());
        assert!(reg.ids().len() >= 8);
    }
}
