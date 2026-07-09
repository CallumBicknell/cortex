//! SQLite pool open + migrations.

use crate::error::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::Path;
use tracing::info;

/// Open (or create) a SQLite database and run migrations.
pub async fn open_sqlite(path: impl AsRef<Path>) -> Result<SqlitePool> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Use a file URL; sqlx expects sqlite:path form.
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    migrate(&pool).await?;
    info!(path = %path.display(), "sqlite memory store ready");
    Ok(pool)
}

/// Apply embedded SQL migrations (idempotent).
pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    let sql = include_str!("../../../migrations/001_init.sql");
    for stmt in sql_statements(sql) {
        sqlx::query(&stmt).execute(pool).await?;
    }
    Ok(())
}

/// Strip line comments and split into SQL statements.
fn sql_statements(sql: &str) -> Vec<String> {
    let mut cleaned = String::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }

    cleaned
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;
    use tempfile::tempdir;

    #[test]
    fn statements_include_sessions_table() {
        let sql = include_str!("../../../migrations/001_init.sql");
        let stmts = sql_statements(sql);
        assert!(
            stmts
                .iter()
                .any(|s| s.contains("CREATE TABLE IF NOT EXISTS sessions")),
            "stmts={stmts:?}"
        );
        assert!(
            stmts.len() >= 8,
            "expected multiple statements, got {}",
            stmts.len()
        );
    }

    #[tokio::test]
    async fn open_and_migrate() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let pool = open_sqlite(&path).await.unwrap();
        let row =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='sessions'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(row.is_some(), "sessions table missing");
        let name: String = row.unwrap().try_get("name").unwrap();
        assert_eq!(name, "sessions");
        // Second open is idempotent.
        let _ = open_sqlite(&path).await.unwrap();
        pool.close().await;
    }
}
