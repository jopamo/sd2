use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use walkdir::WalkDir;

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let ty = entry.file_type();
        if ty.is_dir() {
            continue;
        }
        let rel = entry.path().strip_prefix("src").unwrap();
        let dst_path = dst.as_ref().join(rel);
        if let Some(parent) = dst_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), dst_path)?;
    }
    Ok(())
}

#[test]
fn test_dogfood_self_refactor() {
    let temp_dir = TempDir::new().unwrap();
    let src_copy = temp_dir.path().join("src_copy");
    
    // Copy real src to temp
    copy_dir_all("src", &src_copy).expect("Failed to copy src directory");

    // Collect all files in src_copy
    let mut input_data = Vec::new();
    for entry in WalkDir::new(&src_copy) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            input_data.extend_from_slice(entry.path().to_str().unwrap().as_bytes());
            input_data.push(0);
        }
    }

    // Perform a project-wide rename: "Result" -> "SdResult"
    // We use word boundaries to avoid replacing "FileResult" -> "FileSdResult"
    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("Result")
       .arg("SdResult")
       .arg("--files0")
       .arg("--word-regexp")
       .write_stdin(input_data);

    let output = cmd.output().unwrap();
    
    if !output.status.success() {
        eprintln!("Dogfooding failed: {}", String::from_utf8_lossy(&output.stderr));
        panic!("Command failed");
    }

    let stdout = String::from_utf8(output.stdout).unwrap();
    println!("Dogfood output:\n{}", stdout);

    // Verify changes in a specific file, e.g., main.rs
    let main_rs = src_copy.join("main.rs");
    let content = fs::read_to_string(&main_rs).unwrap();
    
    // Check if "Result" was replaced
    assert!(content.contains("fn try_main() -> SdResult<i32>"));
    // Check if "FileResult" was NOT replaced (due to word-regexp)
    // Actually FileResult is in reporter.rs, let's check reporter.rs
    
    let reporter_rs = src_copy.join("reporter.rs");
    let rep_content = fs::read_to_string(&reporter_rs).unwrap();
    
    // "pub struct FileResult" should still exist
    assert!(rep_content.contains("pub struct FileResult"));
    // "pub fn add_result(&mut self, result: FileResult)" should exist
    assert!(rep_content.contains("result: FileResult"));
}
