use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_dot_slash_prefix_vs_glob() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/match.txt"), "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.current_dir(dir.path())
        .arg("foo")
        .arg("bar")
        .arg("--glob-include")
        .arg("src/*.txt")
        .arg("./src/match.txt") // Explicit ./ prefix
        .assert()
        .success();

    // If glob matched, it should be replaced
    assert_eq!(
        fs::read_to_string(dir.path().join("src/match.txt")).unwrap(),
        "bar",
        "Failed to match ./src/match.txt against src/*.txt"
    );
}

#[test]
fn test_absolute_path_vs_relative_glob() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    let abs_path = dir.path().join("src/match2.txt");
    fs::write(&abs_path, "foo").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stedi"));
    cmd.current_dir(dir.path())
        .arg("foo")
        .arg("bar")
        .arg("--glob-include")
        .arg("src/*.txt")
        .arg(abs_path.to_str().unwrap()) // Absolute path
        .assert()
        .success();

    // If glob matched, it should be replaced
    assert_eq!(
        fs::read_to_string(&abs_path).unwrap(),
        "bar",
        "Failed to match absolute path against relative glob src/*.txt"
    );
}
