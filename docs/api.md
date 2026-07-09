# HTTP API

Cortex exposes a JSON HTTP API via:

```bash
cargo run -p cortex-cli -- serve --bind 127.0.0.1:8080
cargo run -p cortex-cli -- serve --token "$CORTEX_API_TOKEN" --max-turns 24
```

Default bind: `127.0.0.1:8080`. Intended for local/dev use; enable a token for
anything beyond loopback.

## Auth

If `--token` / `CORTEX_API_TOKEN` is set, protected routes require:

```http
Authorization: Bearer <token>
```

or

```http
x-api-key: <token>
```

`GET /health` is always open.

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | no | Liveness |
| GET | `/v1/info` | yes* | Workspace / config summary |
| GET | `/v1/models` | yes* | Model aliases |
| GET | `/v1/tools` | yes* | Registered tools |
| GET | `/v1/sessions?limit=20` | yes* | Recent sessions |
| GET | `/v1/sessions/:id` | yes* | Session + messages |
| POST | `/v1/runs` | yes* | Run an agent task |

\* “yes” only when a token is configured; otherwise open.

### POST /v1/runs

```json
{
  "prompt": "Add a README section about the API",
  "model": "default",
  "session_id": null,
  "yolo": true,
  "max_turns": 16,
  "skills": ["coding", "rust"]
}
```

Response:

```json
{
  "session_id": "…",
  "run_id": "…",
  "status": "succeeded",
  "turns": 3,
  "final_message": "…",
  "duration_ms": 1234,
  "error": null,
  "tool_results": [
    { "name": "read_file", "is_error": false, "output": "…" }
  ]
}
```

Runs use the same `AgentLoop`, skills, summaries, and SQLite persistence as the CLI.

## Python SDK

See [`sdks/python/README.md`](../sdks/python/README.md):

```python
from cortex import CortexClient
with CortexClient() as c:
    print(c.run("hello", yolo=True).final_message)
```

## Crate

`cortex-api` builds the axum `Router` (`cortex_api::router` / `serve`).

## Not yet

- Streaming SSE for token deltas
- WebSocket event stream
- Multi-tenant auth / RBAC
- OpenAPI document generation
- Horizontal scaling (single-process SQLite)
