//! Heuristic skill selection (no hard-coded modes).

use crate::registry::SkillRegistry;
use crate::skill::Skill;
use cortex_workspace::ProjectInfo;
use std::collections::BTreeSet;

/// Result of selecting skills for a task.
#[derive(Debug, Clone)]
pub struct SkillSelection {
    /// Active skills (includes always-on).
    pub skills: Vec<Skill>,
    /// Union of tool names from active skills.
    pub tools: Vec<String>,
    /// Prompt ids to inject.
    pub prompts: Vec<String>,
    /// Skill ids chosen for logging.
    pub skill_ids: Vec<String>,
}

/// Select skills for a user prompt + optional project fingerprint + explicit overrides.
///
/// Selection rules:
/// 1. Always include `always_on` skills.
/// 2. If `explicit` is non-empty, also include those skill ids (when registered).
/// 3. Otherwise, score skills by tag matches against prompt + project languages/tooling.
/// 4. Include any skill with score > 0 (capped) plus always-on.
pub fn select_skills(
    registry: &SkillRegistry,
    prompt: &str,
    project: Option<&ProjectInfo>,
    explicit: &[String],
) -> SkillSelection {
    let mut active_ids: BTreeSet<String> = registry.always_on().into_iter().map(|s| s.id).collect();

    if !explicit.is_empty() {
        for id in explicit {
            if registry.get(id).is_some() {
                active_ids.insert(id.clone());
            }
        }
    } else {
        let haystack = build_haystack(prompt, project);
        let mut scored: Vec<(String, i32)> = registry
            .all()
            .into_iter()
            .filter(|s| !s.always_on)
            .map(|s| {
                let score = score_skill(&s, &haystack);
                (s.id, score)
            })
            .filter(|(_, score)| *score > 0)
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        // Cap opportunistic skills to avoid dumping every pack.
        for (id, _) in scored.into_iter().take(6) {
            active_ids.insert(id);
        }
    }

    let skills: Vec<Skill> = active_ids
        .iter()
        .filter_map(|id| registry.get(id).cloned())
        .collect();

    let mut tools: BTreeSet<String> = BTreeSet::new();
    let mut prompts: BTreeSet<String> = BTreeSet::new();
    // Core system prompt always available via prompts catalog separately.
    for s in &skills {
        for t in &s.tools {
            tools.insert(t.clone());
        }
        for p in &s.prompts {
            prompts.insert(p.clone());
        }
    }

    let skill_ids: Vec<String> = skills.iter().map(|s| s.id.clone()).collect();
    SkillSelection {
        skills,
        tools: tools.into_iter().collect(),
        prompts: prompts.into_iter().collect(),
        skill_ids,
    }
}

fn build_haystack(prompt: &str, project: Option<&ProjectInfo>) -> String {
    let mut parts = vec![prompt.to_ascii_lowercase()];
    if let Some(p) = project {
        parts.push(p.languages.join(" ").to_ascii_lowercase());
        parts.push(p.package_managers.join(" ").to_ascii_lowercase());
        parts.push(p.key_files.join(" ").to_ascii_lowercase());
        if let Some(t) = &p.test_command {
            parts.push(t.to_ascii_lowercase());
        }
    }
    parts.join(" ")
}

fn score_skill(skill: &Skill, haystack: &str) -> i32 {
    let mut score = 0i32;
    // Match skill id as a word-ish substring.
    if haystack.contains(&skill.id.to_ascii_lowercase()) {
        score += 3;
    }
    for tag in &skill.tags {
        let t = tag.to_ascii_lowercase();
        if haystack.contains(&t) {
            score += 2;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::SkillRegistry;
    use cortex_workspace::ProjectInfo;

    #[test]
    fn always_on_coding() {
        let reg = SkillRegistry::with_builtins();
        let sel = select_skills(&reg, "hello", None, &[]);
        assert!(sel.skill_ids.contains(&"coding".to_string()));
        assert!(sel.tools.contains(&"read_file".to_string()));
    }

    #[test]
    fn prompt_selects_solidity() {
        let reg = SkillRegistry::with_builtins();
        let sel = select_skills(&reg, "audit this solidity contract with forge", None, &[]);
        assert!(sel.skill_ids.contains(&"solidity".to_string()));
        assert!(sel.prompts.iter().any(|p| p.contains("solidity")));
    }

    #[test]
    fn project_selects_rust() {
        let reg = SkillRegistry::with_builtins();
        let project = ProjectInfo {
            languages: vec!["rust".into()],
            package_managers: vec!["cargo".into()],
            test_command: Some("cargo test".into()),
            lint_command: None,
            key_files: vec!["Cargo.toml".into()],
        };
        let sel = select_skills(&reg, "fix the bug", Some(&project), &[]);
        assert!(sel.skill_ids.contains(&"rust".to_string()));
    }

    #[test]
    fn explicit_override() {
        let reg = SkillRegistry::with_builtins();
        let sel = select_skills(&reg, "hello", None, &["git".into(), "web".into()]);
        assert!(sel.skill_ids.contains(&"git".to_string()));
        assert!(sel.skill_ids.contains(&"web".to_string()));
        assert!(sel.tools.contains(&"git_status".to_string()));
        assert!(sel.tools.contains(&"http_request".to_string()));
    }

    #[test]
    fn selects_frontend_design() {
        let reg = SkillRegistry::with_builtins();
        let sel = select_skills(
            &reg,
            "make this landing page look distinctive and polish the UI",
            None,
            &[],
        );
        assert!(
            sel.skill_ids.contains(&"frontend_design".to_string()),
            "got {:?}",
            sel.skill_ids
        );
    }

    #[test]
    fn selects_skill_creator() {
        let reg = SkillRegistry::with_builtins();
        let sel = select_skills(&reg, "create a skill for our release checklist", None, &[]);
        assert!(
            sel.skill_ids.contains(&"skill_creator".to_string()),
            "got {:?}",
            sel.skill_ids
        );
    }
}
