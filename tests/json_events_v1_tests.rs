use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::fs;

fn run_stedi_json(args: &[&str]) -> Vec<Value> {
    let mut cmd = cargo_bin_cmd!("stedi");
    let output = cmd.args(args).arg("--format=json").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn test_json_v1_fields_run_end_committed() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let args = vec!["hello", "goodbye", file_path.to_str().unwrap()];
    let events = run_stedi_json(&args);

    let end_wrapper = events.last().unwrap();
    let end = &end_wrapper["run_end"];

    // Check new fields
    assert_eq!(end["committed"], true);
    assert!(end["duration_ms"].is_number());
    assert!(end["total_processed"].is_number());
}

#[test]
fn test_json_v1_fields_run_end_committed_dry_run() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let args = vec!["hello", "goodbye", file_path.to_str().unwrap(), "--dry-run"];
    let events = run_stedi_json(&args);

    let end_wrapper = events.last().unwrap();
    let end = &end_wrapper["run_end"];

    // Check committed is false for dry run
    assert_eq!(end["committed"], false);
}

#[test]
fn test_json_v1_fields_file_is_virtual() {
    let mut cmd = cargo_bin_cmd!("stedi");
    let output = cmd
        .arg("hello")
        .arg("goodbye")
        .arg("--format=json")
        .arg("--stdin-text")
        .write_stdin("hello world")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let events: Vec<Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| l.starts_with("{"))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let file_event = &events[1]["file"];
    assert_eq!(file_event["type"], "success");
    assert_eq!(file_event["is_virtual"], true);
    assert_eq!(file_event["diff_is_binary"], false);
}

#[test]
fn test_json_v1_fields_file_not_virtual() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let args = vec!["hello", "goodbye", file_path.to_str().unwrap()];
    let events = run_stedi_json(&args);

    let file_event = &events[1]["file"];
    assert_eq!(file_event["type"], "success");
    assert_eq!(file_event["is_virtual"], false);
    assert_eq!(file_event["diff_is_binary"], false);
}

#[test]
fn test_json_v1_fields_error_code() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    // Don't create the file, so it fails with NotFound

    let args = vec!["hello", "goodbye", file_path.to_str().unwrap()];
    let events = run_stedi_json(&args);

    let file_event = &events[1]["file"];
    assert_eq!(file_event["type"], "error");
    assert_eq!(file_event["code"], "E_NOT_FOUND");
}
