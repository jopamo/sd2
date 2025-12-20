use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_operation_delete() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("delete.txt");
    fs::write(&file_path, "hello world foo bar").unwrap();

    let manifest_path = temp_dir.path().join("manifest.json");
    let manifest = serde_json::json!({
        "files": [file_path.to_str().unwrap()],
        "operations": [
            {
                "type": "delete",
                "find": "foo "
            }
        ]
    });
    fs::write(&manifest_path, manifest.to_string()).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("apply")
        .arg("--manifest")
        .arg(manifest_path.to_str().unwrap());

    cmd.assert().success();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello world bar");
}

#[test]
fn test_operation_replace_expand() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("expand.txt");
    fs::write(&file_path, "key: value").unwrap();

    let manifest_path = temp_dir.path().join("manifest.json");
    let manifest = serde_json::json!({
        "files": [file_path.to_str().unwrap()],
        "operations": [
            {
                "type": "replace",
                "find": r"(\w+): (\w+)",
                "with": "$2=$1",
                "expand": true
            }
        ]
    });
    fs::write(&manifest_path, manifest.to_string()).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("apply")
        .arg("--manifest")
        .arg(manifest_path.to_str().unwrap());

    cmd.assert().success();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "value=key");
}

#[test]
fn test_operation_delete_regex() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("delete_regex.txt");
    fs::write(&file_path, "remove 123 numbers").unwrap();

    let manifest_path = temp_dir.path().join("manifest.json");
    let manifest = serde_json::json!({
        "files": [file_path.to_str().unwrap()],
        "operations": [
            {
                "type": "delete",
                "find": r"\d+\s+",
                "literal": false
            }
        ]
    });
    fs::write(&manifest_path, manifest.to_string()).unwrap();

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("apply")
        .arg("--manifest")
        .arg(manifest_path.to_str().unwrap());

    cmd.assert().success();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "remove numbers");
}
