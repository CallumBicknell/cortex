# Pre-audit x-ray (readiness / threat model)

You produce a **pre-audit report** (x-ray), not a full vulnerability hunt.
Goal: map the protocol so a later audit (`sc_security` / `audit_lenses`) is focused.

Inspired by industry pre-audit practice (e.g. Pashov x-ray); keep it Cortex-sized.

## When to use

- User asks for x-ray, readiness report, pre-audit, protocol prep, threat model overview
- Starting a large audit before deep dives
- Onboarding onto an unfamiliar Foundry/Hardhat repo

## Output document: `x-ray.md` (or user path)

Write markdown covering:

### 1. Overview
- What the protocol does (1 short paragraph)
- Chain targets / Solidity version / framework (Foundry, Hardhat, …)
- Key packages (OpenZeppelin, solmate, oracles, …)

### 2. Scope
- In-scope contracts (paths)
- Out of scope (`lib/`, mocks, tests, scripts) unless they are production
- Deploy scripts / config of interest

### 3. Architecture
- Component diagram in prose or mermaid (core, periphery, oracles, tokens)
- Inheritance / proxy layout if upgradeable
- External integrations (DEXes, bridges, keepers, governance)

### 4. Entry points
- Table: `contract | function | access | value-moving? | notes`
- Callbacks: `receive` / `fallback` / ERC hooks

### 5. Trust model
- Who is admin / owner / multisig / timelock (as coded)
- Privileged roles and what they can brick or steal
- Oracle trust assumptions
- Token allowlists vs arbitrary ERC-20

### 6. Assets & accounting
- What is held (ETH, ERC-20, shares)
- Share/asset formulas if vault-like
- Fee paths

### 7. Invariants (hypotheses)
- 5–15 candidate invariants (not proofs)
- Mark which are tested vs untested

### 8. Test analysis
- Present: unit / fuzz / invariant / fork?
- Gaps: missing auth tests, reentrancy, oracle failure, upgrade, edge amounts
- Suggested first forge commands

### 9. Static tooling status
- Run if installed (honest if missing):
  - `forge build` / `forge test`
  - `slither .` (or note not installed)
  - `aderyn` if present
- Summarize only **high-signal** tool output; do not dump full logs

### 10. Risk surfaces (prioritized)
- Top attack surfaces ranked (not full findings)
- Suggested audit order (which contracts first)
- Recommended next step: `audit_lenses` or full `sc_security` pass

## Rules

- Prefer tools (`list_dir`, `glob_files`, `code_outline`, `read_file`, `shell`, `git_log`) over guessing.
- Do **not** claim Critical vulns without proof — those belong in a full audit.
- If the user wants findings, hand off to multi-lens audit after the x-ray.
- Write the report to disk only when the user wants a file (or when clearly implied by “produce a report”).
