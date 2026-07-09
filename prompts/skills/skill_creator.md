# Skill creator

Create, improve, and evaluate Cortex skills (capability packs). Inspired by
Anthropic’s skill-creator workflow, adapted to Cortex’s model:

- Skills = `id` + description + tools + tags + optional prompts
- Persist with `skill_save` / `skill_promote` under `.cortex/skills/`
- Measure with `cortex eval` fixtures when outputs are checkable

## When to use

Use this guidance whenever the user wants to:

- turn a successful workflow into a reusable skill
- draft or rewrite a skill description (triggering accuracy matters)
- improve an existing learned skill after feedback
- design eval prompts / assertions for a skill
- decide tools, tags, and notes for `skill_save`

Be a bit “pushy”: prefer proposing a concrete skill draft rather than only
discussing abstract process.

## Process

### 1. Capture intent

From the conversation (and by asking if needed):

1. What should the skill enable the agent to do?
2. When should it trigger? (phrases, project types, file patterns)
3. Expected output shape (code, docs, checklist, commands, …)
4. Are results objectively checkable? If yes, plan evals.

If the user said “turn this into a skill”, extract tools used, steps, corrections,
and I/O formats from history first, then confirm.

### 2. Interview lightly

Ask about edge cases, dependencies, success criteria. Match jargon to the user:
explain “assertion” / “JSON” only if they seem non-technical.

### 3. Write the skill

Prefer Cortex’s on-disk form (via tools):

```text
skill_save:
  id: snake_case_id
  description: what it does AND when to use it (pushy, specific)
  tools: [relevant tool names]
  tags: [trigger keywords]
  notes: why / evolution log
  promote: false  # true when stable
```

**Description quality:** include both *what* and *when*. Prefer concrete triggers
(“whenever the user mentions dashboards, metrics, or charts”) over vague ones.
Avoid under-triggering.

**Body guidance** (put durable instructions in `notes` or a future prompt file
under `prompts/skills/`):

- Imperative steps; explain *why*, not only MUST/NEVER
- Keep lean; remove untested fluff
- Generalize beyond the 1–2 examples you iterated on
- Prefer tool lists that match real Cortex tools (`read_file`, `shell`, …)

### 4. Test cases

For objective skills, draft 2–3 realistic user prompts (casual, specific, with
paths/context). For subjective skills (design, writing), prefer human review.

Map to Cortex evals when useful:

```toml
# evals/<skill_id>_smoke.toml
id = "<skill_id>_smoke"
prompt = "…"
expect_contains = ["…"]
# optional mock steps if offline
```

Run: `cortex eval run --dir evals` (or the workspace evals folder).

### 5. Iterate

1. Save skill draft with `skill_save`
2. Run agent with `--skills <id>` (or tags) on test prompts
3. Collect failures / user feedback into notes
4. `skill_save` again or `skill_promote` when trusted
5. Repeat until feedback is empty or the user is satisfied

### 6. Description polish

If the skill under-triggers:

- broaden tags with near-miss phrasings
- put edge-case triggers into the description
- avoid making should-not-trigger cases “obviously irrelevant”

### Communication

Default to clear, concrete language. Brief definitions OK. Do not create skills
that surprise the user with malicious or deceptive intent.

## Cortex tools for this skill

- `skill_list` — inventory learned packs
- `skill_save` — create/update
- `skill_promote` — mark trusted + bump score
- File tools — draft longer prompt markdown under `prompts/skills/` if needed
- `shell` — run `cortex eval` when appropriate
