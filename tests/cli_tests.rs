use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_validate_only_no_files() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.arg("apply")
        .arg("--validate-only")
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

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
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

#[test]
fn test_stdin_paths() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_stdin.txt");
    fs::write(&file_path, "hello foo world").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--stdin-paths")
        .write_stdin(format!("{}\n", file_path.to_str().unwrap()))
        .assert()
        .success()
        .stdout(predicate::str::contains("Processed 1 files"));
    
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello bar world");
}

#[test]
fn test_stdin_text() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.arg("apply")
        .arg("foo")
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

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    // \0 delimiter
    let input = format!("{}\0", file_path.to_str().unwrap());
    
    cmd.arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--files0")
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
    let json_input = format!(
        r#"{{"type":"begin","path":{{"text":"{}"}}}}
{{"type":"match","path":{{"text":"{}"}},"lines":{{"text":"hello foo world"}},"line_number":1,"absolute_offset":0,"submatches":[{{"match_text":"foo","start":6,"end":9}}]}}
{{"type":"end","path":{{"text":"{}"}},"binary_offset":null,"stats":{{"elapsed":{{"secs":0,"nanos":0,"human":"0s"}},"searches":1,"searches_with_match":1,"matches":1,"matched_lines":1}}}}
"#,
        file_path.to_str().unwrap(),
        file_path.to_str().unwrap(),
        file_path.to_str().unwrap()
    );

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.arg("apply")
        .arg("foo")
        .arg("bar")
        .arg("--rg-json")
        .write_stdin(json_input)
        .assert()
        .success()
        .stdout(predicate::str::contains("Processed 1 files"));
    
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello bar world");
}
