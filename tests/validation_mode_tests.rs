use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_validation_mode_strict_fail() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("strict.txt");
    fs::write(&file_path, "hello world").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg(r"(\w+)")
        .arg("$1bad") // Ambiguous
        .arg(file_path.to_str().unwrap())
        .arg("--expand")
        .arg("--regex")
        .arg("--format=diff")
        .arg("--validation-mode=strict")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Ambiguous capture group reference",
        ));
}

#[test]
fn test_validation_mode_warn_rewrite() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("warn.txt");
    fs::write(&file_path, "hello world").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg(r"(\w+)")
        .arg("$1bad") // Ambiguous
        .arg(file_path.to_str().unwrap())
        .arg("--expand")
        .arg("--regex")
        .arg("--format=diff")
        .arg("--validation-mode=warn")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "WARN: Ambiguous capture group reference",
        ));

    let content = fs::read_to_string(&file_path).unwrap();
    // hello -> hellobad, world -> worldbad
    assert_eq!(content, "hellobad worldbad");
}

#[test]
fn test_validation_mode_none_silent() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("none.txt");
    fs::write(&file_path, "hello world").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg(r"(\w+)")
        .arg("$1bad") // Ambiguous, treated as group '1bad' by regex (empty)
        .arg(file_path.to_str().unwrap())
        .arg("--expand")
        .arg("--regex")
        .arg("--format=diff")
        .arg("--validation-mode=none")
        .assert()
        .success()
        .stderr(predicate::str::contains("Ambiguous capture group reference").not()); // No warning

    let content = fs::read_to_string(&file_path).unwrap();
    // regex treats $1bad as non-existent group -> empty string
    // hello -> "", world -> ""
    assert_eq!(content, " ");
}

#[test]
fn test_validation_mode_default_strict() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("default.txt");
    fs::write(&file_path, "hello world").unwrap();

    // Default should be strict
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg(r"(\w+)")
        .arg("$1bad")
        .arg(file_path.to_str().unwrap())
        .arg("--expand")
        .arg("--regex")
        .arg("--format=diff")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Ambiguous capture group reference",
        ));
}
