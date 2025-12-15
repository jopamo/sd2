use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;
#[cfg(unix)]
use std::os::unix::fs::symlink;

#[test]
#[cfg(unix)]
fn test_symlinks_follow_default() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");
    
    fs::write(&target, "foo").unwrap();
    symlink(&target, &link).unwrap();

    let mut cmd = Command::cargo_bin("sd2").unwrap();
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

    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("foo")
       .arg("bar")
       .arg("--symlinks")
       .arg("skip")
       .arg(link.to_str().unwrap())
       .assert()
       // Even if skipped, it might return success if no other errors occur. 
       // If total files processed > 0 but no matches (because skipped), 
       // it might exit 1 depending on how we handle "no matches found" logic vs "skipped".
       // engine.rs says: if pipeline.require_match && report.replacements == 0 -> policy violation.
       // But by default require_match is false.
       // report.exit_code() returns 1 if modified == 0 && total > 0.
       // If we skip, is it counted in 'total'? 
       // In engine.rs: report.add_result(result) is called. result.skipped is Some.
       // In reporter.rs: self.total += 1.
       // So total > 0. modified == 0. So exit code should be 1.
       .failure(); 

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

    let mut cmd = Command::cargo_bin("sd2").unwrap();
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

    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("foo")
       .arg("bar")
       .arg("--symlinks")
       .arg("error")
       .arg(link.to_str().unwrap())
       .assert()
       .failure(); 
}
