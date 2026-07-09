# Memory: summaries + embeddings

Cortex keeps **session history** in SQLite and adds two long-context aids:

1. **Rolling conversation summaries** — when a session grows large, older turns
   are folded into a short summary injected as system context.
2. **Local vector memory** — workspace files can be indexed with a deterministic
   local embedder and searched via `memory_search` or the CLI.

## Conversation summaries

### When they run

During `cortex run` / `cortex chat`, the agent loop calls
`maybe_summarize` when:

- message count ≥ 32 (default), or
- estimated history tokens ≥ 8000

Older messages (all but the last 16) are summarized with the chat model.
If the LLM call fails, an **extractive** bullet summary is used instead.

### Persistence

Summaries are stored in the `summaries` table (`scope = "rolling"`) and reloaded
on the next turn for the same session.

```bash
# Force-summarize a session
cortex memory summarize <session-id>
cortex memory summarize <session-id> --extractive
```

### Config (code)

`AgentLoopConfig.summarize` (`SummarizeConfig`):

| Field | Default | Meaning |
|-------|---------|---------|
| `enabled` | true | Master switch |
| `message_threshold` | 32 | Trigger by count |
| `token_threshold` | 8000 | Trigger by tokens |
| `keep_recent` | 16 | Verbatim tail |
| `use_llm` | true | Prefer model summary |

## Vector index

Embeddings live in SQLite (`embeddings` table, migration `002_embeddings.sql`).
Vectors are JSON arrays of `f32` (no sqlite-vss required).

### Local embedder

`cortex_memory::local_embed` — 64-d hash/bag-of-tokens embedder. Offline, deterministic.
Fine for demos and keyword-ish retrieval; swap for a provider embedder later for quality.

### CLI

```bash
# Index workspace text files (≤256KB each, ignores binary-ish)
cortex memory index
cortex memory index --max-files 400 --clear

# Search
cortex memory search "authentication middleware"
cortex memory search "jwt" -k 10

# Stats
cortex memory stats
```

Collection name = absolute workspace path.

### Agent tool

`memory_search` is registered when the workspace DB can be opened:

```text
memory_search { "query": "how is config loaded?", "top_k": 5 }
```

Enable via skill: `--skills memory` (or tags: rag, embeddings, retrieve).

```bash
cortex memory index
cortex run "What does the plugin host do?" --skills memory,coding --yolo
```

## Schema notes

- `summaries(id, session_id, scope, content, created_at)` — from `001_init.sql`
- `embeddings(...)` — from `002_embeddings.sql`

## Not yet

- Provider-backed embeddings (OpenAI `/embeddings`) for indexing
- Automatic re-index on file change
- Hybrid BM25 + vector ranking
- Cross-session global memory
