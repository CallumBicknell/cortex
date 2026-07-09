# Frontend design

Guidance for distinctive, intentional UI—not templated AI defaults. Use when
building or reshaping web/app interfaces, landing pages, dashboards, or design
systems in code.

## Stance

Act as a design lead at a small studio: every product gets a visual identity that
could not be mistaken for anyone else’s. Make deliberate, opinionated choices
about palette, type, and layout. Take one real aesthetic risk you can justify.

## Ground it in the subject

If the brief is vague, pin it first:

- concrete subject
- audience
- the page’s single job

Use the product’s own materials, instruments, artifacts, and vernacular. Prefer
real content from the brief over lorem-ipsum decoration.

## Design principles

**Hero is a thesis.** Open with the most characteristic thing in the subject’s
world (headline, image, live demo, interactive moment)—not a generic stat-row
plus gradient unless that truly fits.

**Typography carries personality.** Pair display and body faces deliberately;
set a clear type scale. Avoid the same “safe” pairing you would use on every
project.

**Structure is information.** Numbering, eyebrows, dividers, and labels should
encode something true about the content—not decorate it. Numbered 01/02/03 only
if order actually matters.

**Motion with purpose.** Prefer one orchestrated moment over scattershot
effects. Respect `prefers-reduced-motion`. Sometimes less is more.

**Match complexity to the vision.** Maximalism needs craft; minimalism needs
precision.

**Copy is design material.** Write from the user’s side of the screen. Active
voice; plain verbs; sentence case. Errors explain how to fix. Empty states invite
action. Labels name what people control, not internal system jargon.

## Process

1. **Brainstorm a short plan** (token system):
   - Color: 4–6 named hex values
   - Type: display + body (+ utility if needed)
   - Layout: one-sentence concept + simple ASCII wireframe
   - Signature: the single memorable element
2. **Critique the plan** against the brief. If any part is a generic default you
   would ship for any similar page, revise it and say why.
3. **Build** from the plan; derive every color/type decision from tokens.
4. **Self-critique** as you go (screenshot via browser tools if available).
   Before finishing: remove one accessory (Chanel rule).

### Avoid AI-default clusters (unless the brief asks)

1. Warm cream (~#F4F1EA) + high-contrast serif + terracotta accent
2. Near-black + single acid-green/vermilion accent
3. Broadsheet: hairline rules, zero radius, dense columns

These are legitimate for some briefs; they are not free defaults when the axis
is open.

## Implementation notes (code)

- Watch CSS specificity wars (type selectors fighting utility classes)
- Responsive down to mobile; visible keyboard focus
- Prefer real project content and routes over placeholder marketing fluff
- Keep boldness in the signature element; quiet discipline everywhere else

## Cortex tools that help

- File tools + `apply_patch` for components/styles
- `shell` for install/build/lint (`npm`, `pnpm`, Vite, etc.)
- `browser_*` for visual check when a CDP browser is available
- `code_outline` / `workspace_symbols` to navigate large frontends
