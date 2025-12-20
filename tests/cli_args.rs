use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_validate_only_no_files() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("--validate-only")
        .arg("foo")
        .arg("bar")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No input sources specified"));
}

#[test]
fn test_validate_only_with_file_dry_run() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello foo world").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("--validate-only")
        .arg("--format=diff")
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

#[test]
fn test_stdin_paths() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_stdin.txt");
    fs::write(&file_path, "hello foo world").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("foo")
        .arg("bar")
        .arg("--stdin-paths")
        .arg("--format=diff")
        .write_stdin(format!("{}\n", file_path.to_str().unwrap()))
        .assert()
        .success()
        .stdout(predicate::str::contains("Processed 1 files"));

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello bar world");
}

#[test]
fn test_stdin_text() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("foo")
        .arg("bar")
        .arg("--stdin-text")
        .write_stdin("hello foo world")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello bar world"));
}

#[test]
fn test_files0() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_files0.txt");
    fs::write(&file_path, "hello foo world").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    // \0 delimiter
    let input = format!("{}\0", file_path.to_str().unwrap());

    cmd.arg("foo")
        .arg("bar")
        .arg("--files0")
        .arg("--format=diff")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("Processed 1 files"));

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello bar world");
}

#[test]
fn test_rg_json() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_rg.txt");
    fs::write(&file_path, "hello foo world").unwrap();

    // Construct rg-json input that points to this file
    // Correct ripgrep JSON structure has "data" field
    let p = file_path.to_str().unwrap();
    let begin = format!(r#"{{"type":"begin","data":{{"path":{{"text":"{}"}}}}}}"#, p);
    let match_event = format!(
        r#"{{"type":"match","data":{{"path":{{"text":"{}"}},"lines":{{"text":"hello foo world"}},"line_number":1,"absolute_offset":0,"submatches":[{{"match_text":"foo","start":6,"end":9}}]}}}}"#,
        p
    );
    let end = format!(
        r#"{{"type":"end","data":{{"path":{{"text":"{}"}},"binary_offset":null,"stats":{{"elapsed":{{"secs":0,"nanos":0,"human":"0s"}},"searches":1,"searches_with_match":1,"matches":1,"matched_lines":1}}}}}}"#,
        p
    );
    let json_input = format!("{begin}\n{match_event}\n{end}\n");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("foo")
        .arg("bar")
        .arg("--rg-json")
        .arg("--format=diff")
        .write_stdin(json_input)
        .assert()
        .success()
        .stdout(predicate::str::contains("Processed 1 files"));

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello bar world");
}

#[test]
fn test_limit_alias() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_limit.txt");
    fs::write(&file_path, "foo foo foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.arg("foo")
        .arg("bar")
        .arg("--limit")
        .arg("1")
        .arg(file_path.to_str().unwrap())
        .assert()
        .success();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "bar foo foo");
}
