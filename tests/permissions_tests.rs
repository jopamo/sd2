use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
#[cfg(unix)]
fn test_permissions_preserve_default() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");

    fs::write(&file, "foo").unwrap();
    // Set 0o755
    let p = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&file, p).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg(file.to_str().unwrap())
        .assert()
        .success();

    let meta = fs::metadata(&file).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o755);
}

#[test]
#[cfg(unix)]
fn test_permissions_fixed() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");

    fs::write(&file, "foo").unwrap();
    // Set 0o644 initially
    let p = fs::Permissions::from_mode(0o644);
    fs::set_permissions(&file, p).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--permissions")
        .arg("fixed")
        .arg("--mode")
        .arg("755")
        .arg(file.to_str().unwrap())
        .assert()
        .success();

    let meta = fs::metadata(&file).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o755);
}

#[test]
fn test_permissions_fixed_missing_mode_fails() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, "foo").unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--permissions")
        .arg("fixed")
        .arg(file.to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicates::str::contains("--mode <OCTAL> is required"));
}
