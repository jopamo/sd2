use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_range_single_line() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_range.txt");
    fs::write(&file_path, "foo\nfoo\nfoo\nfoo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("foo")
        .arg("bar")
        .arg("--range")
        .arg("2")
        .arg(file_path.to_str().unwrap())
        .assert()
        .success();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "foo\nbar\nfoo\nfoo");
}

#[test]
fn test_range_start_end() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_range_2.txt");
    fs::write(&file_path, "foo\nfoo\nfoo\nfoo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("foo")
        .arg("bar")
        .arg("--range")
        .arg("2:3")
        .arg(file_path.to_str().unwrap())
        .assert()
        .success();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "foo\nbar\nbar\nfoo");
}

#[test]
fn test_range_start_unbounded() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_range_3.txt");
    fs::write(&file_path, "foo\nfoo\nfoo\nfoo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("foo")
        .arg("bar")
        .arg("--range")
        .arg("3:")
        .arg(file_path.to_str().unwrap())
        .assert()
        .success();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "foo\nfoo\nbar\nbar");
}
