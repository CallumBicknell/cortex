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

#[test]
fn setup_and_doctor_use_cortex_home() {
    let home = tempdir().expect("home");
    let ws = tempdir().expect("ws");
    cargo_bin_cmd!("cortex")
        .current_dir(ws.path())
        .env("CORTEX_HOME", home.path())
        .args(["setup"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cortex home"));
    assert!(home.path().join("models.toml").is_file());
    assert!(home.path().join("data").is_dir());

    cargo_bin_cmd!("cortex")
        .current_dir(ws.path())
        .env("CORTEX_HOME", home.path())
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cortex doctor"))
        .stdout(predicate::str::contains("models_ok"));
}

#[test]
fn run_works_outside_monorepo_with_home_only() {
    let home = tempdir().expect("home");
    let ws = tempdir().expect("ws");
    let db = home.path().join("data/cortex-test.db");
    cargo_bin_cmd!("cortex")
        .current_dir(ws.path())
        .env("CORTEX_HOME", home.path())
        .env("CORTEX_DATABASE", &db)
        .args([
            "run",
            "hello outside monorepo",
            "--json",
            "--yolo",
            "--max-turns",
            "2",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("final_message"));
}

#[test]
fn init_web3_scaffolds_mcp_and_instructions() {
    let home = tempdir().expect("home");
    let ws = tempdir().expect("ws");
    cargo_bin_cmd!("cortex")
        .current_dir(ws.path())
        .env("CORTEX_HOME", home.path())
        .args(["init", "--web3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Foundry MCP"));
    assert!(ws.path().join(".cortex/mcp.toml").is_file());
    assert!(ws.path().join(".cortex/instructions.md").is_file());
    assert!(ws.path().join("AGENTS.md").is_file());
}

#[test]
fn update_dry_run() {
    cargo_bin_cmd!("cortex")
        .args(["update", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("install.sh"));
}

#[test]
fn run_loads_agents_md() {
    let home = tempdir().expect("home");
    let ws = tempdir().expect("ws");
    let db = home.path().join("data/cortex-test.db");
    std::fs::write(
        ws.path().join("AGENTS.md"),
        "# Rules\n- always mention pineapple\n",
    )
    .unwrap();
    cargo_bin_cmd!("cortex")
        .current_dir(ws.path())
        .env("CORTEX_HOME", home.path())
        .env("CORTEX_DATABASE", &db)
        .args(["run", "say hi", "--yolo", "--max-turns", "2"])
        .assert()
        .success()
        .stderr(predicate::str::contains("project instructions"));
}
