# Cortex Python SDK

HTTP client for a running Cortex API (`cortex serve`).

## Install

```bash
cd sdks/python
pip install -e ".[dev]"   # or: pip install -e .
```

## Quick start

Terminal 1 — start the API against a workspace:

```bash
cargo run -p cortex-cli -- serve --bind 127.0.0.1:8080
# optional: --token secret   or CORTEX_API_TOKEN=secret
```

Terminal 2 — Python:

```python
from cortex import CortexClient, Agent

with CortexClient("http://127.0.0.1:8080") as client:
    print(client.health())
    print(client.models())
    result = client.run("Summarize this repository", yolo=True, max_turns=8)
    print(result.status, result.final_message)

    agent = Agent(name="docs", client=client, skills=["coding"])
    r2 = agent.run("List the crates in this monorepo")
    print(r2.final_message)
```

Async:

```python
import asyncio
from cortex import AsyncCortexClient

async def main():
    async with AsyncCortexClient() as client:
        print(await client.health())
        r = await client.run("hello", yolo=True)
        print(r.final_message)

asyncio.run(main())
```

## API surface

| Method | HTTP |
|--------|------|
| `health()` | `GET /health` |
| `info()` | `GET /v1/info` |
| `models()` | `GET /v1/models` |
| `tools()` | `GET /v1/tools` |
| `sessions(limit=)` | `GET /v1/sessions` |
| `get_session(id)` | `GET /v1/sessions/:id` |
| `run(prompt, ...)` | `POST /v1/runs` |

Auth: pass `api_key=` to send `Authorization: Bearer …`.

## Local tools

`@tool` still builds local Python callables for your own apps. Remote agent runs
use **server-side** Cortex tools unless/until plugin registration over HTTP lands.

## Tests

```bash
pip install -e .
pytest
```
