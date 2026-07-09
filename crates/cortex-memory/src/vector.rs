//! Local embedding helpers and SQLite-backed vector store.

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Default dimensionality for the built-in local embedder.
pub const LOCAL_EMBED_DIMS: usize = 64;
/// Model id recorded for the local hash embedder.
pub const LOCAL_EMBED_MODEL: &str = "local-hash-v1";

/// One stored embedding chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingChunk {
    /// Row id.
    pub id: String,
    /// Logical collection (e.g. workspace path or "session:{id}").
    pub collection: String,
    /// Stable source key (file path, message id, …).
    pub source_id: String,
    /// Kind: file | message | note.
    pub source_kind: String,
    /// Original text (truncated when indexed).
    pub content: String,
    /// Embedding vector.
    pub embedding: Vec<f32>,
    /// Model that produced the vector.
    pub model: String,
}

/// Search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredChunk {
    /// Chunk.
    pub chunk: EmbeddingChunk,
    /// Cosine similarity in [-1, 1].
    pub score: f32,
}

/// Deterministic bag-of-tokens local embedder (no network).
///
/// Good enough for tests and offline RAG demos; swap for a real provider model
/// when quality matters.
pub fn local_embed(text: &str) -> Vec<f32> {
    local_embed_dims(text, LOCAL_EMBED_DIMS)
}

/// Local embedder with explicit dimensions.
pub fn local_embed_dims(text: &str, dims: usize) -> Vec<f32> {
    let dims = dims.max(8);
    let mut v = vec![0.0f32; dims];
    let lower = text.to_ascii_lowercase();
    for token in lower.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-') {
        if token.is_empty() || token.len() > 48 {
            continue;
        }
        let h = hash_token(token);
        let idx = (h as usize) % dims;
        let sign = if h & 1 == 0 { 1.0 } else { -1.0 };
        v[idx] += sign;
        // Bigram boost with next-char for short tokens.
        if token.len() >= 2 {
            let h2 = hash_token(&token[..2]);
            v[(h2 as usize) % dims] += 0.5 * sign;
        }
    }
    // Character n-grams for short / code-y text.
    let bytes = lower.as_bytes();
    if bytes.len() >= 3 {
        for w in bytes.windows(3) {
            let h = hash_bytes(w);
            v[(h as usize) % dims] += 0.25;
        }
    }
    l2_normalize(&mut v);
    v
}

/// Cosine similarity; returns 0 if either vector is empty or zero-norm.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

fn l2_normalize(v: &mut [f32]) {
    let mut n = 0.0f32;
    for x in v.iter() {
        n += *x * *x;
    }
    if n == 0.0 {
        return;
    }
    let inv = 1.0 / n.sqrt();
    for x in v.iter_mut() {
        *x *= inv;
    }
}

fn hash_token(s: &str) -> u64 {
    hash_bytes(s.as_bytes())
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    // FNV-1a 64-bit
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// SQLite-backed vector memory.
#[derive(Clone)]
pub struct VectorStore {
    pool: SqlitePool,
}

impl VectorStore {
    /// Wrap a migrated pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert a chunk (unique on collection + source_id).
    pub async fn upsert(
        &self,
        collection: &str,
        source_id: &str,
        source_kind: &str,
        content: &str,
        embedding: &[f32],
        model: &str,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let emb_json = serde_json::to_string(embedding)?;
        let created = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO embeddings (
                id, collection, source_id, source_kind, content,
                embedding_json, dims, model, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(collection, source_id) DO UPDATE SET
                content = excluded.content,
                embedding_json = excluded.embedding_json,
                dims = excluded.dims,
                model = excluded.model,
                source_kind = excluded.source_kind,
                created_at = excluded.created_at
            "#,
        )
        .bind(&id)
        .bind(collection)
        .bind(source_id)
        .bind(source_kind)
        .bind(content)
        .bind(&emb_json)
        .bind(embedding.len() as i64)
        .bind(model)
        .bind(&created)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Delete all chunks in a collection.
    pub async fn clear_collection(&self, collection: &str) -> Result<u64> {
        let r = sqlx::query("DELETE FROM embeddings WHERE collection = ?")
            .bind(collection)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected())
    }

    /// Count chunks in a collection.
    pub async fn count(&self, collection: &str) -> Result<u64> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM embeddings WHERE collection = ?")
            .bind(collection)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("c")? as u64)
    }

    /// Brute-force cosine top-k within a collection.
    pub async fn search(
        &self,
        collection: &str,
        query: &[f32],
        top_k: usize,
    ) -> Result<Vec<ScoredChunk>> {
        let rows = sqlx::query(
            r#"
            SELECT id, collection, source_id, source_kind, content, embedding_json, model
            FROM embeddings
            WHERE collection = ?
            "#,
        )
        .bind(collection)
        .fetch_all(&self.pool)
        .await?;

        let mut scored = Vec::with_capacity(rows.len());
        for row in rows {
            let emb_json: String = row.try_get("embedding_json")?;
            let embedding: Vec<f32> = serde_json::from_str(&emb_json)?;
            let score = cosine_similarity(query, &embedding);
            scored.push(ScoredChunk {
                chunk: EmbeddingChunk {
                    id: row.try_get("id")?,
                    collection: row.try_get("collection")?,
                    source_id: row.try_get("source_id")?,
                    source_kind: row.try_get("source_kind")?,
                    content: row.try_get("content")?,
                    embedding,
                    model: row.try_get("model")?,
                },
                score,
            });
        }
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(top_k.max(1));
        Ok(scored)
    }

    /// Index free text with the local embedder.
    pub async fn index_text_local(
        &self,
        collection: &str,
        source_id: &str,
        source_kind: &str,
        content: &str,
    ) -> Result<String> {
        let emb = local_embed(content);
        self.upsert(
            collection,
            source_id,
            source_kind,
            content,
            &emb,
            LOCAL_EMBED_MODEL,
        )
        .await
    }

    /// Search using the local embedder for the query string.
    pub async fn search_text_local(
        &self,
        collection: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<ScoredChunk>> {
        let q = local_embed(query);
        self.search(collection, &q, top_k).await
    }
}

/// Format search hits for LLM context injection.
pub fn format_retrieval_section(hits: &[ScoredChunk], max_chars: usize) -> String {
    if hits.is_empty() {
        return String::new();
    }
    let mut out = String::from("## Retrieved memory\n");
    for (i, h) in hits.iter().enumerate() {
        let block = format!(
            "### hit {} (score={:.3}, {})\n{}\n\n",
            i + 1,
            h.score,
            h.chunk.source_id,
            truncate(&h.chunk.content, 800)
        );
        if out.len() + block.len() > max_chars {
            break;
        }
        out.push_str(&block);
    }
    out
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sqlite;
    use tempfile::tempdir;

    #[test]
    fn similar_text_scores_higher() {
        let a = local_embed("authentication login password jwt token");
        let b = local_embed("user login auth password");
        let c = local_embed("red blue green watercolor painting");
        let sim_ab = cosine_similarity(&a, &b);
        let sim_ac = cosine_similarity(&a, &c);
        assert!(sim_ab > sim_ac, "sim_ab={sim_ab} sim_ac={sim_ac}");
    }

    #[tokio::test]
    async fn upsert_and_search() {
        let dir = tempdir().unwrap();
        let pool = open_sqlite(dir.path().join("v.db")).await.unwrap();
        let store = VectorStore::new(pool);
        store
            .index_text_local(
                "ws",
                "src/auth.rs",
                "file",
                "fn login() { verify_password() }",
            )
            .await
            .unwrap();
        store
            .index_text_local("ws", "src/paint.rs", "file", "fn blend_colors(r, g, b)")
            .await
            .unwrap();
        let hits = store
            .search_text_local("ws", "password authentication login", 2)
            .await
            .unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].chunk.source_id, "src/auth.rs");
        assert_eq!(store.count("ws").await.unwrap(), 2);
    }
}
