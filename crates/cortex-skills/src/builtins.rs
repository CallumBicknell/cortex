//! Builtin skill packs.

use crate::skill::Skill;

/// All builtin skills shipped with Cortex.
pub fn builtin_skills() -> Vec<Skill> {
    vec![
        Skill::new("coding", "Core file editing and repository navigation")
            .tools([
                "read_file",
                "write_file",
                "edit_file",
                "list_dir",
                "glob_files",
            ])
            .prompts(["coding", "security"])
            .tags(["coding", "files", "edit"])
            .always_on(),
        Skill::new("shell", "Run shell commands in the workspace")
            .tools(["shell"])
            .tags(["shell", "cli", "command"]),
        Skill::new("git", "Inspect and record git history")
            .tools(["git_status", "git_diff", "git_log", "git_add", "git_commit"])
            .prompts(["skills/git"])
            .tags(["git", "commit", "diff", "vcs"]),
        Skill::new("web", "Fetch public HTTP resources")
            .tools(["http_request"])
            .prompts(["skills/web"])
            .tags(["web", "http", "docs", "fetch", "url"]),
        Skill::new("testing", "Run and fix automated tests")
            .tools(["shell", "read_file", "edit_file"])
            .prompts(["skills/testing"])
            .tags(["test", "testing", "pytest", "cargo test", "ci"]),
        Skill::new("rust", "Rust / Cargo projects")
            .tools([
                "shell",
                "read_file",
                "write_file",
                "edit_file",
                "glob_files",
            ])
            .prompts(["skills/rust"])
            .tags(["rust", "cargo", "clippy"]),
        Skill::new("python", "Python projects")
            .tools([
                "shell",
                "read_file",
                "write_file",
                "edit_file",
                "glob_files",
            ])
            .prompts(["skills/python"])
            .tags(["python", "pytest", "pip", "ruff"]),
        Skill::new("javascript", "JavaScript / TypeScript projects")
            .tools([
                "shell",
                "read_file",
                "write_file",
                "edit_file",
                "glob_files",
            ])
            .prompts(["skills/javascript"])
            .tags(["javascript", "typescript", "node", "npm", "pnpm", "yarn"]),
        Skill::new(
            "solidity",
            "Solidity / smart-contract workflows (Foundry-oriented)",
        )
        .tools([
            "shell",
            "read_file",
            "write_file",
            "edit_file",
            "glob_files",
        ])
        .prompts(["skills/solidity", "security"])
        .tags([
            "solidity",
            "foundry",
            "forge",
            "ethereum",
            "smart contract",
            "slither",
            "audit",
        ]),
        Skill::new("review", "Code review and quality focus")
            .tools(["read_file", "glob_files", "list_dir"])
            .prompts(["review"])
            .tags(["review", "audit", "quality"]),
    ]
}
