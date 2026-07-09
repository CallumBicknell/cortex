-- Vector memory / embeddings (Phase 12)
-- Vectors stored as JSON arrays of f32 for portability (no sqlite-vss required).

CREATE TABLE IF NOT EXISTS embeddings (
    id              TEXT PRIMARY KEY NOT NULL,
    collection      TEXT NOT NULL,
    source_id       TEXT NOT NULL,
    source_kind     TEXT NOT NULL,
    content         TEXT NOT NULL,
    embedding_json  TEXT NOT NULL,
    dims            INTEGER NOT NULL,
    model           TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    UNIQUE (collection, source_id)
);

CREATE INDEX IF NOT EXISTS idx_embeddings_collection
    ON embeddings(collection);

CREATE INDEX IF NOT EXISTS idx_summaries_session
    ON summaries(session_id, created_at);
