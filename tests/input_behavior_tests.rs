use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_repeated_inputs_no_transaction() {
    // Behavior: Sequential application.
    // 1. Read A, replace -> B, write B.
    // 2. Read B, replace -> C, write C.
    
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    fs::write(&file_path, "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.current_dir(dir.path())
        .arg("foo")
        .arg("bar")
        .arg("file.txt")
        .arg("file.txt") // Repeated
        .assert()
        .success();

    // First pass: foo -> bar.
    // Second pass: bar -> bar (no match for foo).
    // Result: bar.
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "bar");
}

#[test]
fn test_repeated_inputs_chained_no_transaction() {
    // Verify sequential processing with chaining
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    fs::write(&file_path, "A").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.current_dir(dir.path())
        .arg("--regex")
        .arg("[A-Z]")
        .arg("X") // Replace any capital letter with X
        .arg("file.txt")
        .arg("file.txt")
        .assert()
        .success();

    // 1. Read A. Replace A->X. Write X.
    // 2. Read X. Replace X->X. Write X.
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "X");
}

#[test]
fn test_repeated_inputs_transaction_all() {
    // Behavior: Parallel-ish application (read from original).
    // 1. Read A. Replace -> B. Stage B.
    // 2. Read A (original). Replace -> B. Stage B.
    // Commit: Write B.
    
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    fs::write(&file_path, "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.current_dir(dir.path())
        .arg("foo")
        .arg("bar")
        .arg("--transaction")
        .arg("all")
        .arg("file.txt")
        .arg("file.txt")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(&file_path).unwrap(), "bar");
}

#[test]
fn test_glob_include_exclude_precedence() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "foo").unwrap();
    fs::write(dir.path().join("b.txt"), "foo").unwrap();
    fs::write(dir.path().join("c.md"), "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.current_dir(dir.path())
        .arg("foo")
        .arg("bar")
        .arg("--glob-include")
        .arg("*.txt")   // Include a.txt, b.txt
        .arg("--glob-exclude")
        .arg("b.txt")   // Exclude b.txt
        .arg("a.txt")
        .arg("b.txt")
        .arg("c.md")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "bar"); // Matches include, not excluded
    assert_eq!(fs::read_to_string(dir.path().join("b.txt")).unwrap(), "foo"); // Matches include, but excluded -> Skip
    assert_eq!(fs::read_to_string(dir.path().join("c.md")).unwrap(), "foo");  // No match include -> Skip
}

#[test]
fn test_glob_exclude_only() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "foo").unwrap();
    fs::write(dir.path().join("b.txt"), "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.current_dir(dir.path())
        .arg("foo")
        .arg("bar")
        .arg("--glob-exclude")
        .arg("b.txt")
        .arg("a.txt")
        .arg("b.txt")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "bar");
    assert_eq!(fs::read_to_string(dir.path().join("b.txt")).unwrap(), "foo");
}
