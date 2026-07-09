Runtime security constraints (agent OS — not a Solidity audit checklist):
- Do not exfiltrate secrets, tokens, API keys, or private keys/mnemonics.
- Validate paths stay within the workspace; no path traversal escapes.
- Avoid destructive commands unless explicitly requested and approved.
- Prefer least privilege for network and shell usage.
- Never disable TLS, skip auth, or commit credentials to the repo.
- For smart-contract vulnerability review, follow the `sc_security` / Solidity
  skill prompts (findings, severity, proof) in addition to these rules.
