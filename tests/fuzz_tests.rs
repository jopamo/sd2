use assert_cmd::cargo::cargo_bin_cmd;
use proptest::prelude::*;

proptest! {
    // Fuzz the rg-json input parser
    #[test]
    fn fuzz_rg_json_input(s in r"\PC*") {
        let mut cmd = cargo_bin_cmd!("stedi");
        // invalid inputs should fail gracefully (exit code 1) or succeed with 0 replacements
        // but never panic or exit with a weird signal.
        let assert = cmd
            .arg("foo")
            .arg("bar")
            .arg("--rg-json")
            .write_stdin(s.as_bytes())
            .assert();

        // We accept failure (invalid json) or success.
        // We mainly care that it didn't segfault or panic.
        // assert_cmd checks for status, but we want to allow 1.
        let output = assert.get_output();
        if !output.status.success() {
             // Ensure it's not a panic (usually printed to stderr)
             let stderr = String::from_utf8_lossy(&output.stderr);
             if stderr.contains("panicked at") {
                 panic!("Panicked on input: {}", stderr);
             }
        }
    }

    // Fuzz stdin-text content
    #[test]
    fn fuzz_stdin_text(s in r"\PC*") {
        let mut cmd = cargo_bin_cmd!("stedi");
        let assert = cmd
            .arg("foo")
            .arg("bar")
            .arg("--stdin-text")
            .write_stdin(s.as_bytes())
            .assert()
            .success();

        // This should always succeed (exit 0) because text replacement never fails on valid utf8 string
        // (proptest generates Strings, so valid UTF8).
        // Unless we hit memory limits, but for small strings it's fine.

        let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
        // If s contains "foo", stdout should change.
        if s.contains("foo") {
             assert_ne!(stdout, s);
        }
    }
}
