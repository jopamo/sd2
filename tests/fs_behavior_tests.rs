use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::PredicateBooleanExt;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::tempdir;

#[test]
#[cfg(unix)]
fn test_symlinks_follow_default() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");

    fs::write(&target, "foo").unwrap();
    symlink(&target, &link).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg(link.to_str().unwrap()) // Pass the link
        .assert()
        .success();

    // Target should be modified
    assert_eq!(fs::read_to_string(&target).unwrap(), "bar");
}

#[test]
#[cfg(unix)]
fn test_symlinks_skip() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");

    fs::write(&target, "foo").unwrap();
    symlink(&target, &link).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--symlinks")
        .arg("skip")
        .arg(link.to_str().unwrap())
        .assert()
        // Even if skipped, it might return success if no other errors occur.
        .success();

    // Target should NOT be modified
    assert_eq!(fs::read_to_string(&target).unwrap(), "foo");
}

#[test]
#[cfg(unix)]
fn test_symlinks_skip_with_other_files() {
    // If we process a valid file AND a skipped symlink, we should exit 0 (if valid file modified)
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");
    let regular = dir.path().join("regular.txt");

    fs::write(&target, "foo").unwrap();
    fs::write(&regular, "foo").unwrap();
    symlink(&target, &link).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--symlinks")
        .arg("skip")
        .arg(link.to_str().unwrap())
        .arg(regular.to_str().unwrap())
        .assert()
        .success();

    // Target should NOT be modified (via link)
    assert_eq!(fs::read_to_string(&target).unwrap(), "foo");
    // Regular should be modified
    assert_eq!(fs::read_to_string(&regular).unwrap(), "bar");
}

#[test]
#[cfg(unix)]
fn test_symlinks_error() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");

    fs::write(&target, "foo").unwrap();
    symlink(&target, &link).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--symlinks")
        .arg("error")
        .arg(link.to_str().unwrap())
        .assert()
        .failure();
}

#[test]
fn test_binary_skip_default() {
    let dir = tempdir().unwrap();
    let bin_file = dir.path().join("bin.dat");

    // Create a file with a null byte
    fs::write(&bin_file, b"foo\0bar").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg(bin_file.to_str().unwrap())
        .assert()
        // Should default to skip, so no error (unless all skipped is an error, which we decided is exit code 1 if total > 0 modified == 0)
        .success();

    // Content should remain unchanged
    let content = fs::read(&bin_file).unwrap();
    assert_eq!(content, b"foo\0bar");
}

#[test]
fn test_binary_error() {
    let dir = tempdir().unwrap();
    let bin_file = dir.path().join("bin.dat");

    fs::write(&bin_file, b"foo\0bar").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--binary")
        .arg("error")
        .arg(bin_file.to_str().unwrap())
        .assert()
        .failure()
        // The reporter prints errors to stdout in human format
        .stdout(
            predicates::str::contains("Binary file detected")
                .or(predicates::str::contains("ERROR")),
        );
}

#[test]
fn test_binary_skip_with_other_files() {
    let dir = tempdir().unwrap();
    let bin_file = dir.path().join("bin.dat");
    let txt_file = dir.path().join("text.txt");

    fs::write(&bin_file, b"foo\0bar").unwrap();
    fs::write(&txt_file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg(bin_file.to_str().unwrap())
        .arg(txt_file.to_str().unwrap())
        .assert()
        .success();

    // Binary unchanged
    assert_eq!(fs::read(&bin_file).unwrap(), b"foo\0bar");
    // Text changed
    assert_eq!(fs::read_to_string(&txt_file).unwrap(), "bar");
}
