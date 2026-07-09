# Shared finding schema (all lenses)

For each issue:

```markdown
### [SEVERITY|LEAD] Title
- **Contract / function:** `File.sol:name`
- **Bug class:** short-tag
- **Root cause:** one sentence
- **Impact:** who loses what
- **Proof:** path, numbers, or quoted code
- **Minimal fix:** smallest correct change
- **Confidence:** high | medium | low
```

Severity: Critical | High | Medium | Low | Informational.
Use **LEAD** (not severity) when proof is incomplete.

Rules:
- Prefer fewer high-quality items over laundry lists.
- Do not invent files that are not in the source bundle or workspace.
- Skip `lib/`, mocks, and pure test helpers unless they define production surface.
