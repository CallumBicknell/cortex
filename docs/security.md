# Security

Cortex is designed for **local coding agents** with safe-by-default controls.

## Layers

1. **Policy** (`config/security.toml` or `.cortex/security.toml`) — tool allow/deny/ask, path sandbox, HTTP host blocks, shell deny patterns, env scrub list.
2. **Approval** — interactive CLI prompts for `ask` tools; `--yolo` auto-allows (still blocks catastrophic shell patterns).
3. **Path sandbox** — filesystem tools resolve paths under the workspace root.
4. **Secrets redaction** — API keys / tokens redacted in approval prompts and human CLI output.
5. **Audit log** — approval decisions persisted to SQLite `permissions_audit` when a session DB is available.

## CLI

```bash
cortex security show
cortex run "…" --yolo          # approve-all (dangerous patterns still blocked)
```

Environment:

- `CORTEX_SECURITY_CONFIG` — path to a custom security.toml

## Shell

- Working directory defaults to workspace root
- Secret env vars stripped from the child process
- Patterns such as `rm -rf /` are hard-denied even under yolo

## HTTP

- Private / link-local / metadata hosts blocked by default
- Optional allow-list via `http_allow_hosts`
