-- Cortex SQLite schema (Phase 6)

CREATE TABLE IF NOT EXISTS sessions (
    id            TEXT PRIMARY KEY NOT NULL,
    workspace     TEXT NOT NULL,
    model         TEXT NOT NULL,
    status        TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id            TEXT PRIMARY KEY NOT NULL,
    session_id    TEXT NOT NULL,
    seq           INTEGER NOT NULL,
    payload_json  TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    UNIQUE (session_id, seq),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, seq);

CREATE TABLE IF NOT EXISTS tool_calls (
    id            TEXT PRIMARY KEY NOT NULL,
    session_id    TEXT NOT NULL,
    message_id    TEXT,
    name          TEXT NOT NULL,
    input_json    TEXT NOT NULL,
    status        TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tool_results (
    id            TEXT PRIMARY KEY NOT NULL,
    tool_call_id  TEXT NOT NULL,
    session_id    TEXT NOT NULL,
    output        TEXT NOT NULL,
    is_error      INTEGER NOT NULL DEFAULT 0,
    duration_ms   INTEGER,
    created_at    TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS events (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT,
    kind            TEXT NOT NULL,
    payload_json    TEXT NOT NULL,
    correlation_id  TEXT,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_session ON events(session_id, created_at);

CREATE TABLE IF NOT EXISTS checkpoints (
    id               TEXT PRIMARY KEY NOT NULL,
    session_id       TEXT NOT NULL,
    label            TEXT,
    loop_state_json  TEXT NOT NULL,
    message_count    INTEGER NOT NULL,
    created_at       TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_checkpoints_session ON checkpoints(session_id, created_at);

CREATE TABLE IF NOT EXISTS artifacts (
    id            TEXT PRIMARY KEY NOT NULL,
    session_id    TEXT NOT NULL,
    kind          TEXT NOT NULL,
    name          TEXT NOT NULL,
    sha256        TEXT,
    size_bytes    INTEGER,
    created_at    TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS summaries (
    id            TEXT PRIMARY KEY NOT NULL,
    session_id    TEXT NOT NULL,
    scope         TEXT NOT NULL,
    content       TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS permissions_audit (
    id            TEXT PRIMARY KEY NOT NULL,
    session_id    TEXT,
    tool_name     TEXT NOT NULL,
    decision      TEXT NOT NULL,
    detail_json   TEXT,
    created_at    TEXT NOT NULL
);
