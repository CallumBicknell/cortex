//! CLI smoke tests (mock provider, no network).

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

#[test]
fn tools_list_prints_read_file() {
    cargo_bin_cmd!("cortex")
        .arg("tools")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("read_file"));
}

#[test]
fn run_mock_json() {
    let root = repo_root();
    cargo_bin_cmd!("cortex")
        .current_dir(&root)
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
