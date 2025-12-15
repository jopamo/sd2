use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_validate_only_no_files() {
    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("apply")
        .arg("--validate-only")
        .arg("foo")
        .arg("bar")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No files specified for processing"));
}

#[test]
fn test_validate_only_with_file_dry_run() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello foo world").unwrap();

    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("apply")
        .arg("--validate-only")
        .arg("foo")
        .arg("bar")
        .arg(file_path.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("VALIDATION RUN"))
        .stdout(predicate::str::contains("Processed 1 files"));
    
    // Verify file was NOT modified (because validate-only implies dry-run)
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello foo world");
}
