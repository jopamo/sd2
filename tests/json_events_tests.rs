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
fn test_json_golden_path() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let args = vec!["hello", "goodbye", file_path.to_str().unwrap()];
    let events = run_stedi_json(&args);

    assert_eq!(events.len(), 3); // RunStart, File, RunEnd

    // RunStart
    let start_event = &events[0];
    assert!(start_event.get("run_start").is_some());
    let start = &start_event["run_start"];
    assert_eq!(start["schema_version"], "1");
    assert_eq!(start["mode"], "cli");
    assert_eq!(start["policies"]["fail_on_change"], false);

    // File Event
    let file_wrapper = &events[1];
    assert!(file_wrapper.get("file").is_some());
    let file_event = &file_wrapper["file"];
    assert_eq!(file_event["type"], "success");
    // path might differ in format (absolute vs relative) depending on how stedi handles it
    // we passed absolute path, so it should be absolute.
    assert_eq!(file_event["path"], file_path.to_str().unwrap());
    assert_eq!(file_event["modified"], true);
    assert_eq!(file_event["replacements"], 1);

    // RunEnd
    let end_wrapper = &events[2];
    assert!(end_wrapper.get("run_end").is_some());
    let end = &end_wrapper["run_end"];
    assert_eq!(end["total_files"], 1);
    assert_eq!(end["total_modified"], 1);
    assert_eq!(end["total_replacements"], 1);
    assert_eq!(end["has_errors"], false);
    assert_eq!(end["exit_code"], 0);
}

#[test]
fn test_json_skip_reasons() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("binary.bin");
    // Write null byte to make it binary
    fs::write(&bin_path, b"hello\0world").unwrap();

    let args = vec!["hello", "goodbye", bin_path.to_str().unwrap()];
    let events = run_stedi_json(&args);

    let file_event = &events[1]["file"];
    assert_eq!(file_event["type"], "skipped");
    assert_eq!(file_event["reason"], "binary");
}

#[test]
fn test_json_validate_only() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let args = vec![
        "hello",
        "goodbye",
        file_path.to_str().unwrap(),
        "--validate-only",
    ];
    let events = run_stedi_json(&args);

    let start = &events[0]["run_start"];
    assert_eq!(start["validate_only"], true);
    // start["dry_run"] remains false in the report because it reflects the CLI argument,
    // even though internally execution behaves like dry-run.

    let file_event = &events[1]["file"];
    assert_eq!(file_event["type"], "success");
    assert_eq!(file_event["modified"], true); // It found changes

    // Verify file content is unchanged
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello world");
}

#[test]
fn test_json_no_write() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let args = vec![
        "hello",
        "goodbye",
        file_path.to_str().unwrap(),
        "--no-write",
    ];
    let events = run_stedi_json(&args);

    let start = &events[0]["run_start"];
    assert_eq!(start["no_write"], true);

    let file_event = &events[1]["file"];
    assert_eq!(file_event["type"], "success");
    assert_eq!(file_event["modified"], true);

    // Verify file content is unchanged
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello world");
}

#[test]
fn test_json_stdin_text() {
    // For stdin, we need to use Command builder differently than run_stedi_json helper
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

    let start = &events[0]["run_start"];
    assert_eq!(start["input_mode"], "stdin-text");

    let file_event = &events[1]["file"];
    assert_eq!(file_event["type"], "success");
    assert_eq!(file_event["path"], "<stdin>");
    assert_eq!(file_event["modified"], true);
}

#[test]
fn test_json_transaction_staging_failure() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    let file_path = subdir.join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    // Make subdir read-only to prevent creating temp file inside it
    let original_perms = fs::metadata(&subdir).unwrap().permissions();
    let mut perms = original_perms.clone();
    perms.set_readonly(true);
    fs::set_permissions(&subdir, perms).unwrap();

    // Use transaction all
    let args = vec![
        "hello",
        "goodbye",
        file_path.to_str().unwrap(),
        "--transaction=all",
    ];

    let mut cmd = cargo_bin_cmd!("stedi");
    let output_result = cmd.args(&args).arg("--format=json").output();

    // Cleanup: restore permissions so tempdir can be deleted
    fs::set_permissions(&subdir, original_perms).unwrap();
    let output = output_result.unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let events: Vec<Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    assert!(events.len() >= 2);
    // File event should be Error because stage_file failed to create temp file
    let file_event = &events[1]["file"];

    assert_eq!(file_event["type"], "error");
    // Verify it's not success
}
