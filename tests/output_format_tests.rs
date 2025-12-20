use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;

#[test]
fn test_default_output_format_non_tty() {
    // assert_cmd runs without a TTY, so it simulates a pipe/file redirection.
    // Current behavior: Defaults to Diff (Human readable)
    // Desired behavior: Defaults to Json

    // We'll create a dummy file to run against
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("hello")
        .arg("goodbye")
        .arg(&file_path)
        .arg("--dry-run"); // To ensure we get output

    // Verify current behavior (or what we expect BEFORE changes)
    // Ideally this test fails after we make changes, then we update it.
    // Or we write it for the Desired behavior and see it fail now.

    // Let's write for Desired behavior: It should be JSON.
    // JSON output should start with "{" (for Report struct)
    cmd.assert().stdout(predicate::str::starts_with("{"));
}

#[test]
fn test_explicit_format_summary() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "foo bar").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("baz")
        .arg(&file_path)
        .arg("--dry-run")
        .arg("--format=summary");

    // Summary should NOT contain the diff
    // The diff would look like "-foo bar" or "+baz bar" or colored output.
    // In Diff format (print_human), it prints the diff.
    // In Summary format, we want to ensure no diff lines are printed.

    // Note: currently print_human prints the diff. So this test might pass or fail depending on if diff is generated.
    // But we want to ensure "Summary" explicitly excludes diffs even if they are available.

    cmd.assert()
        .stdout(predicate::str::contains("modified (1 replacements)")) // Summary part
        .stdout(predicate::str::contains("@@").not()); // Diff header usually contains @@
}

#[test]
fn test_explicit_format_agent() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("agent.txt");
    fs::write(&file_path, "foo bar").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("baz")
        .arg(&file_path)
        .arg("--dry-run")
        .arg("--format=agent");

    cmd.assert()
        .stdout(predicate::str::contains(format!(
            "<file path=\"{}\">",
            file_path.display()
        )))
        .stdout(predicate::str::contains("-foo bar"))
        .stdout(predicate::str::contains("+baz bar"))
        .stdout(predicate::str::contains("</file>"));
}
