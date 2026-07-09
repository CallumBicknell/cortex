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
                "apply_patch",
                "code_outline",
            ])
            .prompts(["coding", "security"])
            .tags(["coding", "files", "edit", "patch", "outline", "symbols"])
            .always_on(),
        Skill::new("shell", "Run shell commands in the workspace")
            .tools(["shell"])
            .tags(["shell", "cli", "command"]),
        Skill::new("docker", "Run commands in Docker containers")
            .tools(["docker_run"])
            .tags(["docker", "container"]),
        Skill::new("git", "Inspect and record git history")
            .tools(["git_status", "git_diff", "git_log", "git_add", "git_commit"])
            .prompts(["skills/git"])
            .tags(["git", "commit", "diff", "vcs"]),
        Skill::new("web", "Fetch public HTTP resources and search the web")
            .tools(["http_request", "web_search"])
            .prompts(["skills/web"])
            .tags(["web", "http", "docs", "fetch", "url", "search"]),
        Skill::new(
            "memory",
            "Semantic memory search over indexed workspace content",
        )
        .tools(["memory_search"])
        .tags(["memory", "search", "rag", "embeddings", "index", "retrieve"]),
        Skill::new(
            "evolve",
            "Self-evolving skills: list, save, and promote learned capability packs",
        )
        .tools(["skill_list", "skill_save", "skill_promote"])
        .tags([
            "skill",
            "evolve",
            "learn",
            "promote",
            "capability",
            "workflow",
        ]),
        // Adapted from Anthropic skills: skill-creator
        // https://github.com/anthropics/skills/tree/main/skills/skill-creator
        Skill::new(
            "skill_creator",
            "Create, improve, and evaluate Cortex skills (capability packs). \
             Use whenever the user wants to invent a skill, turn a workflow into a skill, \
             rewrite a skill description for better triggering, design skill evals, or \
             iterate skill quality with feedback — even if they say 'make a pack', \
             'capture this process', or 'optimize skill tags' without naming skill_creator.",
        )
        .tools([
            "skill_list",
            "skill_save",
            "skill_promote",
            "read_file",
            "write_file",
            "edit_file",
            "list_dir",
            "glob_files",
            "shell",
        ])
        .prompts(["skills/skill_creator"])
        .tags([
            "skill_creator",
            "skill-creator",
            "create a skill",
            "create skill",
            "write skill",
            "improve skill",
            "skill eval",
            "skill description",
            "capability pack",
            "SKILL.md",
            "workflow capture",
            "turn this into a skill",
            "new skill",
            "skill pack",
        ]),
        // Adapted from Anthropic skills: frontend-design
        // https://github.com/anthropics/skills/tree/main/skills/frontend-design
        Skill::new(
            "frontend_design",
            "Distinctive frontend/UI design guidance (palette, type, layout, copy). \
             Use for new UIs, redesigns, landing pages, dashboards, design systems, \
             CSS/HTML/React/Vue components, or when the user wants the interface to \
             avoid generic AI-looking templates — even if they only say 'make it look \
             better', 'polish the UI', or 'design this page'.",
        )
        .tools([
            "read_file",
            "write_file",
            "edit_file",
            "list_dir",
            "glob_files",
            "apply_patch",
            "shell",
            "code_outline",
            "workspace_symbols",
            "browser_navigate",
            "browser_snapshot",
            "browser_content",
            "browser_click",
            "browser_evaluate",
        ])
        .prompts(["skills/frontend_design"])
        .tags([
            "frontend",
            "frontend-design",
            "frontend_design",
            "ui",
            "ux",
            "css",
            "html",
            "react",
            "vue",
            "svelte",
            "tailwind",
            "landing page",
            "dashboard",
            "design system",
            "typography",
            "visual design",
            "web design",
        ]),
        Skill::new(
            "code_intel",
            "Workspace symbols and definitions via tree-sitter index",
        )
        .tools(["code_outline", "workspace_symbols", "code_definition"])
        .tags(["symbols", "definition", "lsp", "outline", "goto"]),
        Skill::new(
            "browser",
            "Headless browser via CDP (Obscura, Chrome, Chromium, custom)",
        )
        .tools([
            "browser_navigate",
            "browser_evaluate",
            "browser_snapshot",
            "browser_content",
            "browser_click",
            "browser_close",
        ])
        .tags([
            "browser",
            "cdp",
            "obscura",
            "chrome",
            "chromium",
            "puppeteer",
            "playwright",
            "scrape",
            "headless",
        ]),
        Skill::new("testing", "Run and fix automated tests")
            .tools(["shell", "read_file", "edit_file", "apply_patch"])
            .prompts(["skills/testing"])
            .tags(["test", "testing", "pytest", "cargo test", "ci"]),
        Skill::new("rust", "Rust / Cargo projects")
            .tools([
                "shell",
                "read_file",
                "write_file",
                "edit_file",
                "glob_files",
                "apply_patch",
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
                "apply_patch",
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
                "apply_patch",
            ])
            .prompts(["skills/javascript"])
            .tags(["javascript", "typescript", "node", "npm", "pnpm", "yarn"]),
        Skill::new(
            "solidity",
            "Solidity / EVM smart-contract development (Foundry-oriented). \
             Use for implementing contracts, forge tests, remappings, Hardhat, \
             OpenZeppelin integrations, or any .sol / foundry.toml work — \
             including when the user says 'write a vault', 'add a forge test', \
             or 'fix the contract compile'.",
        )
        .tools([
            "shell",
            "read_file",
            "write_file",
            "edit_file",
            "glob_files",
            "list_dir",
            "apply_patch",
            "code_outline",
            "workspace_symbols",
            "code_definition",
        ])
        .prompts(["skills/solidity", "security"])
        .tags([
            "solidity",
            "foundry",
            "forge",
            "ethereum",
            "evm",
            "smart contract",
            "smart contracts",
            "hardhat",
            "openzeppelin",
            "erc20",
            "erc721",
            "erc4626",
            ".sol",
            "foundry.toml",
            "anvil",
            "cast",
        ]),
        // Smart-contract security — methodology aligned with common Web3 audit
        // practice and ethskills.com/security; catalog: https://skills.eth.sh/
        Skill::new(
            "sc_security",
            "Smart-contract security audits and vulnerability finding for Solidity/EVM. \
             Use whenever the user wants an audit, threat model, pre-deploy review, \
             reentrancy/oracle/auth check, multi-lens audit, exploit sketch, Slither pass, \
             or to 'find vulns' / 'security review' contracts — even if they only say \
             'is this safe?', 'check this vault', or 'x-ray this protocol'. Prefer \
             audit_lenses for parallel specialty passes, then dedupe into one report.",
        )
        .tools([
            "shell",
            "read_file",
            "write_file",
            "edit_file",
            "glob_files",
            "list_dir",
            "apply_patch",
            "code_outline",
            "workspace_symbols",
            "code_definition",
            "spawn_subagent",
            "audit_lenses",
            "write_audit_report",
            "http_request",
            "web_search",
            "memory_search",
        ])
        .prompts([
            "skills/sc_security",
            "skills/sc_analyzers",
            "skills/sc_poc",
            "skills/findings_schema",
            "skills/solidity",
            "security",
        ])
        .tags([
            "sc_security",
            "smart contract security",
            "solidity security",
            "audit",
            "security audit",
            "security review",
            "vulnerability",
            "vulnerabilities",
            "vuln",
            "find vulns",
            "reentrancy",
            "slither",
            "mythril",
            "echidna",
            "medusa",
            "aderyn",
            "invariant",
            "threat model",
            "pre-deploy",
            "pre audit",
            "pre-audit",
            "exploit",
            "poc",
            "access control",
            "oracle manipulation",
            "flash loan",
            "delegatecall",
            "upgradeable",
            "uups",
            "erc-4626 inflation",
            "mev",
            "quillshield",
            "pashov",
            "web3 security",
        ]),
        // Pre-audit readiness / threat map (Pashov x-ray–inspired, Cortex-sized)
        Skill::new(
            "sc_xray",
            "Pre-audit x-ray / readiness report for Solidity protocols: scope, \
             entry points, trust model, invariants, test gaps, risk surfaces. \
             Use for 'x-ray', 'pre-audit report', 'readiness', 'protocol prep', \
             or 'summarize this protocol before audit' — not a full vuln dump.",
        )
        .tools([
            "shell",
            "read_file",
            "write_file",
            "edit_file",
            "glob_files",
            "list_dir",
            "code_outline",
            "workspace_symbols",
            "code_definition",
            "git_status",
            "git_log",
            "git_diff",
            "write_audit_report",
            "memory_search",
        ])
        .prompts([
            "skills/sc_xray",
            "skills/sc_analyzers",
            "skills/findings_schema",
            "skills/solidity",
            "security",
        ])
        .tags([
            "sc_xray",
            "x-ray",
            "xray",
            "pre-audit report",
            "readiness report",
            "readiness",
            "protocol prep",
            "audit readiness",
            "threat model overview",
            "summarize this protocol",
            "prep this protocol",
        ]),
        Skill::new("review", "Code review and quality focus (general software)")
            .tools(["read_file", "glob_files", "list_dir", "code_outline"])
            .prompts(["review"])
            .tags(["code review", "quality", "pr review", "review this pr"]),
    ]
}
