//! CLI smoke tests (mock provider, no network).

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::path::PathBuf;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// Isolate SQLite so parallel tests do not race migrations on the shared workspace DB.
fn isolated_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().expect("tempdir");
    let db = dir.path().join("cortex-test.db");
    (dir, db)
}

#[test]
fn tools_list_prints_read_file() {
    let root = repo_root();
    let (_tmp, db) = isolated_db();
    cargo_bin_cmd!("cortex")
        .current_dir(&root)
        .env("CORTEX_DATABASE", &db)
        .arg("tools")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("read_file"))
        .stdout(predicate::str::contains("memory_search"));
}

#[test]
fn run_mock_json() {
    let root = repo_root();
    let (_tmp, db) = isolated_db();
    cargo_bin_cmd!("cortex")
        .current_dir(&root)
        .env("CORTEX_DATABASE", &db)
        .args([
            "run",
            "hello from smoke test",
            "--json",
            "--yolo",
            "--max-turns",
            "2",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("final_message"))
        .stdout(predicate::str::contains("mock"));
}

#[test]
fn models_list() {
    let root = repo_root();
    cargo_bin_cmd!("cortex")
        .current_dir(&root)
        .args(["models", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("default"));
}
