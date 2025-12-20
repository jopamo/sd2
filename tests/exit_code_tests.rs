use assert_cmd::cargo::cargo_bin_cmd;

#[test]
fn test_exit_code_success_replace() {
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--stdin-text")
        .write_stdin("foo")
        .assert()
        .success(); // Exit code 0
}

#[test]
fn test_exit_code_no_matches() {
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--stdin-text")
        .write_stdin("baz")
        .assert()
        .success(); // Exit code 0 (Success even if no matches)
}

#[test]
fn test_exit_code_policy_require_match() {
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--stdin-text")
        .arg("--require-match")
        .write_stdin("baz")
        .assert()
        .code(2); // POLICY_VIOLATION
}

#[test]
fn test_exit_code_policy_fail_on_change() {
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--stdin-text")
        .arg("--fail-on-change")
        .write_stdin("foo")
        .assert()
        .code(2); // POLICY_VIOLATION
}

#[test]
fn test_exit_code_error_invalid_regex() {
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("p(")
        .arg("bar")
        .arg("--stdin-text")
        .arg("--regex")
        .write_stdin("foo")
        .assert()
        .code(1); // ERROR (Input/Runtime error)
}

#[test]
fn test_exit_code_io_error() {
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("/non/existent/file")
        .assert()
        .code(1); // ERROR (File not found)
}
