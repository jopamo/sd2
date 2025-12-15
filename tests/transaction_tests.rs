use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_transaction_file_mode_partial_failure() {
    // In file mode, files are modified as we go.
    // If we have a policy violation at the end, the changes persist.
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "foo").unwrap();
    fs::write(&f2, "foo").unwrap();

    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("foo")
       .arg("bar")
       .arg("--transaction")
       .arg("file")
       .arg("--expect")
       .arg("3") // We have 2 matches, so this will fail
       .arg(f1.to_str().unwrap())
       .arg(f2.to_str().unwrap())
       .assert()
       .failure(); // Exit code 2

    // Changes SHOULD persist in File mode because policy check is post-execution
    assert_eq!(fs::read_to_string(&f1).unwrap(), "bar");
    assert_eq!(fs::read_to_string(&f2).unwrap(), "bar");
}

#[test]
fn test_transaction_all_mode_rollback() {
    // In all mode, changes are staged.
    // If policy violation occurs, nothing should be written.
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "foo").unwrap();
    fs::write(&f2, "foo").unwrap();

    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("foo")
       .arg("bar")
       .arg("--transaction")
       .arg("all")
       .arg("--expect")
       .arg("3") // We have 2 matches, so this will fail
       .arg(f1.to_str().unwrap())
       .arg(f2.to_str().unwrap())
       .assert()
       .failure(); // Exit code 2

    // Changes SHOULD NOT persist
    assert_eq!(fs::read_to_string(&f1).unwrap(), "foo");
    assert_eq!(fs::read_to_string(&f2).unwrap(), "foo");
}

#[test]
fn test_transaction_all_mode_success() {
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    fs::write(&f1, "foo").unwrap();
    fs::write(&f2, "foo").unwrap();

    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("foo")
       .arg("bar")
       .arg("--transaction")
       .arg("all")
       .arg(f1.to_str().unwrap())
       .arg(f2.to_str().unwrap())
       .assert()
       .success();

    // Changes SHOULD persist
    assert_eq!(fs::read_to_string(&f1).unwrap(), "bar");
    assert_eq!(fs::read_to_string(&f2).unwrap(), "bar");
}
