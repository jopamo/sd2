use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_diff_no_trailing_newline() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("no_newline.txt");
    // Write content without a trailing newline
    fs::write(&file, "foo\nbar").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("bar")
        .arg("baz")
        .arg("--format=diff")
        .arg("--dry-run")
        .arg(file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicates::str::contains("-bar"))
        .stdout(predicates::str::contains("+baz"));
}

#[test]
fn test_diff_crlf_preservation() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("crlf.txt");
    // Write content with CRLF
    fs::write(&file, "foo\r\nbar\r\n").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("baz")
        .arg("--format=diff")
        .arg("--dry-run")
        .arg(file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicates::str::contains("-foo"))
        .stdout(predicates::str::contains("+baz"));
}

#[test]
fn test_content_preservation_no_trailing_newline() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("no_newline_apply.txt");
    fs::write(&file, "foo\nbar").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("bar")
        .arg("baz")
        .arg(file.to_str().unwrap())
        .assert()
        .success();

    let content = fs::read_to_string(&file).unwrap();
    assert_eq!(content, "foo\nbaz");
}

#[test]
fn test_content_preservation_crlf() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("crlf_apply.txt");
    fs::write(&file, "foo\r\nbar\r\n").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("baz")
        .arg(file.to_str().unwrap())
        .assert()
        .success();

    let content = fs::read_to_string(&file).unwrap();
    assert_eq!(content, "baz\r\nbar\r\n");
}

#[test]
fn test_diff_exact_output_no_newline() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("no_newline_exact.txt");
    fs::write(&file, "line1\nline2").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    let output = cmd
        .arg("line2")
        .arg("modified")
        .arg("--format=diff")
        .arg("--dry-run")
        .arg(file.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    // We expect the diff to show the change and the "No newline" marker.
    assert!(stdout.contains("-line2"));
    assert!(stdout.contains("\\ No newline at end of file"));
    assert!(stdout.contains("+modified"));

    // Check path header stability
    assert!(stdout.contains("no_newline_exact.txt: modified"));
}
