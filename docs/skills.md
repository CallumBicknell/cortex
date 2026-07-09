# Skills

Skills are **capability packs**, not hard-coded modes.

A skill declares:

- **id** — stable name (`coding`, `rust`, `solidity`, …)
- **description** — for catalogs / optional LLM selection
- **tools** — tool names the pack may use
- **prompts** — markdown prompt ids injected when active
- **tags** — keywords for heuristic matching
- **always_on** — included for every run (e.g. `coding`)

## Builtin packs

| Skill | Always | Tools (summary) |
|-------|--------|-----------------|
| `coding` | yes | read/write/edit/list/glob/outline |
| `shell` | no | shell |
| `git` | no | git_status/diff/log/add/commit |
| `web` | no | http_request, web_search |
| `memory` | no | memory_search |
| `evolve` | no | skill_list/save/promote |
| `skill_creator` | no | skill tools + files + shell (create/eval skills) |
| `frontend_design` | no | files, shell, browser, symbols (UI design) |
| `code_intel` | no | outline / workspace_symbols / definition |
| `browser` | no | CDP browser tools |
| `testing` | no | shell + file tools |
| `rust` | no | cargo-oriented file + shell |
| `python` | no | pytest-oriented |
| `javascript` | no | npm/pnpm/yarn-oriented |
| `solidity` | no | forge-oriented guidance |
| `review` | no | read-only review prompts |

### High-value common packs

- **`skill_creator`** — adapted from [Anthropic skill-creator](https://github.com/anthropics/skills/tree/main/skills/skill-creator): draft → test → iterate → `skill_save` / promote; optional `cortex eval` fixtures.
- **`frontend_design`** — adapted from [Anthropic frontend-design](https://github.com/anthropics/skills/tree/main/skills/frontend-design): intentional visual design, anti-template defaults, plan-then-build.

Prompts: `prompts/skills/skill_creator.md`, `prompts/skills/frontend_design.md`.

Solidity is a **skill**, not “Solidity mode”. Activating it loads related prompts/tools; it does not switch the entire agent into a special global state.

## Selection

1. Always include `always_on` skills.
2. If `--skills a,b` is passed, also include those ids.
3. Otherwise score skills by tag/id matches against:
   - the user prompt
   - project fingerprint (languages, package managers, key files)

Top matches (capped) are activated. Tool schemas sent to the model are the **union** of active skill tools.

## CLI

```bash
cortex skills list
cortex skills select "audit this forge contract"
cortex run "fix rust compile error" --skills rust,testing
```

## Prompts

Markdown lives under `prompts/`:

- `system.md`, `planner.md`, `coding.md`, `review.md`, `security.md`
- `skills/*.md` for pack-specific guidance

`cortex-prompts` embeds builtins at compile time and can also load a workspace `prompts/` directory at runtime.
