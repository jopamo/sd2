use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_output_streams_normal() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--format=summary")
        .arg(file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Processed 1 files, modified 1, 1 replacements.",
        ))
        .stderr("");
}

#[test]
fn test_output_streams_policy_error() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
       .arg("bar")
       .arg("--format=summary")
       .arg("--require-match")
       .arg("nomatch")
       .arg(file.to_str().unwrap())
       .assert()
       .failure()
       .stderr(predicates::str::contains("Policy Error: No matches found"))
       // Depending on implementation, stdout might still have the report summary
       // But the critical part is that the error is in stderr
       ;
}

#[test]
fn test_output_streams_file_error() {
    let dir = tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    let file = subdir.join("file.txt");
    fs::write(&file, "foo").unwrap();

    // Make directory read-only to prevent atomic rename
    let mut perms = fs::metadata(&subdir).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&subdir, perms).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--format=summary")
        .arg(file.to_str().unwrap())
        .assert()
        .failure()
        // Error should be in stderr
        .stderr(predicates::str::contains("ERROR -"))
        // Summary might be in stdout
        .stdout(predicates::str::contains("Processed 1 files"));
}

#[test]
fn test_output_streams_json_stdout_only() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--json")
        .arg(file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicates::str::contains("\"replacements\":1"))
        .stderr("");
}

#[test]
fn test_output_streams_json_errors_stdout_only() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--json")
        .arg("--require-match")
        .arg("nomatch")
        .arg(file.to_str().unwrap())
        .assert()
        .failure()
        .stdout(predicates::str::contains("\"policy_violation\":"))
        .stderr("");
}

#[test]
fn test_output_streams_stdin_text_human() {
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.write_stdin("foo")
        .arg("foo")
        .arg("bar")
        .arg("--stdin-text")
        .assert()
        .success()
        .stdout("bar") // The content
        // Metadata to stderr
        .stderr(predicates::str::contains("Processed 1 files"));
}

#[test]
fn test_output_streams_stdin_text_json() {
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.write_stdin("foo")
        .arg("foo")
        .arg("bar")
        .arg("--stdin-text")
        .arg("--json")
        .assert()
        .success()
        // generated_content should be in JSON
        .stdout(predicates::str::contains("\"generated_content\":\"bar\""))
        .stderr(""); // No metadata on stderr
}
