use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_stdin_text_json_output() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.arg("foo")
       .arg("bar")
       .arg("--stdin-text")
       .arg("--format=json")
       .write_stdin("hello foo world")
       .assert()
       .success()
       .stdout(predicate::str::contains(r#""generated_content":"hello bar world""#));
}

#[test]
fn test_stdin_text_json_no_change() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sd2"));
    cmd.arg("zzz")
       .arg("bar")
       .arg("--stdin-text")
       .arg("--format=json")
       .write_stdin("hello foo world")
       .assert()
       .success()
       .stdout(predicate::str::contains(r#""generated_content":"hello foo world""#)); // Returns original if no change? 
       // Documentation says "generated_content: The full transformed content."
       // If modified is false, it should still return the content for stdin-text mode?
       // Let's check src/engine.rs logic.
}
