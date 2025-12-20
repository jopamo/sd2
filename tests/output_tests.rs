use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_quiet_suppresses_success() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--quiet")
        .arg("--format=summary")
        .arg(file.to_str().unwrap())
        .assert()
        .success()
        .stdout("");
}

#[test]
fn test_quiet_prints_errors() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--quiet")
        .arg("--format=summary")
        .arg("--require-match")
        .arg("baz") // This won't match, triggering policy error
        .arg(file.to_str().unwrap())
        .assert()
        .failure()
        // Policy violation is printed to stderr in print_errors_only
        .stderr(predicates::str::contains("Policy Error: No matches found"));
}

#[test]
fn test_quiet_json_prints_json() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--quiet")
        .arg("--json")
        .arg(file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicates::str::contains("\"replacements\":1"))
        .stderr("");
}

#[test]
fn test_quiet_json_captures_errors_in_json_and_silences_stderr() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--quiet")
        .arg("--json")
        .arg("--require-match")
        .arg("baz") // Won't match
        .arg(file.to_str().unwrap())
        .assert()
        .failure() // Should fail due to policy
        .stdout(predicates::str::contains(
            "\"policy_violation\":\"No matches found (--require-match)\"",
        ))
        .stderr(""); // Stderr should be silent
}
