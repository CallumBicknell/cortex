# Shared findings schema (Solidity security)

Use this schema for **FINDING** and **LEAD** items across `sc_security`,
`audit_lenses`, and analyzer triage.

## Markdown form (primary)

```markdown
### [SEVERITY|LEAD] Title
- **id:** F-001
- **Contract / function:** `File.sol:functionName`
- **Bug class:** reentrancy | access-control | oracle | accounting | upgrade | dos | other
- **Root cause:** one sentence at code level
- **Impact:** who loses what (ETH/tokens/availability)
- **Preconditions:** attacker capabilities / market conditions
- **Proof:** path, numbers, tool detector, or forge test name
- **Minimal fix:** smallest correct change
- **Confidence:** high | medium | low
- **Tools:** manual | slither:Detector | audit_lenses:reentrancy | …
```

## Severity

| Level | Use when |
|-------|----------|
| Critical | Direct theft / permanent freeze of principal / unstoppable brick |
| High | Conditional theft or major DoS with realistic preconditions |
| Medium | Limited loss or harder conditions |
| Low | Best-practice, edge-only, limited impact |
| Informational | Style, gas, docs — no exploit path |
| LEAD | Hypothesis without full proof (not a severity) |

## JSON form (optional machine-readable)

When writing `.cortex/audits/*.json` or tooling output:

```json
{
  "schema_version": 1,
  "findings": [
    {
      "id": "F-001",
      "kind": "finding",
      "severity": "high",
      "title": "Reentrancy on withdraw",
      "contract": "Vault.sol",
      "function": "withdraw",
      "bug_class": "reentrancy",
      "root_cause": "balance zeroed after external call",
      "impact": "attacker drains ETH",
      "preconditions": "contract holds ETH; attacker is contract",
      "proof": "CEI violation; see test/exploit/ReentrancyPoC.t.sol",
      "fix": "zero balance before call; nonReentrant",
      "confidence": "high",
      "tools": ["manual", "audit_lenses:reentrancy"]
    }
  ]
}
```

`kind` is `"finding"` or `"lead"`. Leads omit or null `severity`.

## Dedup key

Merge on `(contract, function, bug_class)` first; keep richest proof and most severe label.
