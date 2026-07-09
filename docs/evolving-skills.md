# Self-evolving skills

Cortex can **learn new capability packs** during work and reuse them later.

## How it works

1. Built-in skills ship in the binary (`coding`, `rust`, `evolve`, …).
2. The agent can call:
   - `skill_list` — show learned skills under `.cortex/skills/`
   - `skill_save` — write a new/updated skill (id, description, tools, tags, notes)
   - `skill_promote` — mark a skill trusted and bump its score
3. On the next run, `SkillRegistry::with_builtins_and_store` loads disk skills so
   auto-selection / `--skills` can activate them.

## Skill file format

`.cortex/skills/my_workflow.toml`:

```toml
origin = "learned"
score = 1
notes = "Worked well for API docs tasks"

[skill]
id = "my_workflow"
description = "Document HTTP endpoints with examples"
tools = ["read_file", "glob_files", "write_file"]
tags = ["docs", "api", "http"]
prompts = []
always_on = false
```

## Agent guidance

| Skill | When |
|-------|------|
| **`evolve`** | Quick save/list/promote of a pack after a task |
| **`skill_creator`** | Full create/improve/eval loop (Anthropic-style skill authoring) |

```bash
cortex run "Turn our API docs workflow into a skill" --skills skill_creator --yolo
cortex run "Improve the api_docs skill description" --skills skill_creator,evolve --yolo
```

Example prompt:

> After finishing, save a skill `api_docs` that captures the tools and tags you used.

See also: [Anthropic skill-creator](https://github.com/anthropics/skills/tree/main/skills/skill-creator)
(source of the process model; Cortex maps packaging to `skill_save` + optional evals).

## CLI

```bash
ls .cortex/skills/
cargo run -p cortex-cli -- skills list   # builtins; learned appear when selected via store
```

Learned skills are selected when:

- Explicit: `--skills my_workflow`
- Heuristic tags match the prompt / project
