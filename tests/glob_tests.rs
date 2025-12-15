use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_glob_include() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("match.txt"), "foo").unwrap();
    fs::write(dir.path().join("ignore.md"), "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.current_dir(dir.path())
        .arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--glob-include")
        .arg("*.txt")
        .arg("match.txt")
        .arg("ignore.md")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(dir.path().join("match.txt")).unwrap(), "bar");
    assert_eq!(fs::read_to_string(dir.path().join("ignore.md")).unwrap(), "foo");
}

#[test]
fn test_glob_exclude() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("match.txt"), "foo").unwrap();
    fs::write(dir.path().join("ignore.md"), "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.current_dir(dir.path())
        .arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--glob-exclude")
        .arg("*.md")
        .arg("match.txt")
        .arg("ignore.md")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(dir.path().join("match.txt")).unwrap(), "bar");
    assert_eq!(fs::read_to_string(dir.path().join("ignore.md")).unwrap(), "foo");
}

#[test]
fn test_glob_include_exclude_combined() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "foo").unwrap();
    fs::write(dir.path().join("b.txt"), "foo").unwrap();
    fs::write(dir.path().join("c.md"), "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.current_dir(dir.path())
        .arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--glob-include")
        .arg("*.txt")
        .arg("--glob-exclude")
        .arg("b.txt")
        .arg("a.txt")
        .arg("b.txt")
        .arg("c.md")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "bar"); // Matches include, not excluded
    assert_eq!(fs::read_to_string(dir.path().join("b.txt")).unwrap(), "foo"); // Matches include, but excluded
    assert_eq!(fs::read_to_string(dir.path().join("c.md")).unwrap(), "foo"); // Does not match include
}

#[test]
fn test_glob_absolute_path_recursive() {
    let dir = tempdir().unwrap();
    let abs_path = dir.path().join("nested").join("deep.txt");
    fs::create_dir_all(abs_path.parent().unwrap()).unwrap();
    fs::write(&abs_path, "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    // Pass absolute path
    // glob "**/*.txt" should match it
    
    cmd.arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--glob-include")
        .arg("**/*.txt")
        .arg(abs_path.to_str().unwrap())
        .assert()
        .success();

    assert_eq!(fs::read_to_string(&abs_path).unwrap(), "bar");
}

#[test]
fn test_glob_absolute_path_basename_match() {
    let dir = tempdir().unwrap();
    let abs_path = dir.path().join("match.txt");
    fs::write(&abs_path, "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    // "*.txt" matches basename even in absolute path (globset behavior)
    
    cmd.arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--glob-include")
        .arg("*.txt")
        .arg(abs_path.to_str().unwrap())
        .assert()
        .success();

    assert_eq!(fs::read_to_string(&abs_path).unwrap(), "bar");
}

#[test]
fn test_glob_absolute_path_extension_mismatch() {
    let dir = tempdir().unwrap();
    let abs_path = dir.path().join("match.md");
    fs::write(&abs_path, "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    
    cmd.arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--glob-include")
        .arg("*.txt")
        .arg(abs_path.to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No input sources specified"));

    assert_eq!(fs::read_to_string(&abs_path).unwrap(), "foo");
}