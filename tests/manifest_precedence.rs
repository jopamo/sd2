use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_manifest_transaction_override() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("foo.txt");
    fs::write(&file_path, "hello world").unwrap();

    let manifest_path = temp_dir.path().join("manifest.json");
    // Manifest sets transaction: "file"
    let manifest = serde_json::json!({
        "files": [file_path.to_str().unwrap()],
        "transaction": "file",
        "operations": [
            {
                "type": "replace",
                "find": "world",
                "with": "universe"
            }
        ]
    });
    fs::write(&manifest_path, manifest.to_string()).unwrap();

    // Run without CLI override -> should use "file"
    // To verify, we can inspect JSON output if it exposes transaction mode.
    // The event stream "run_start" event should contain "transaction_mode".

    let mut cmd = Command::cargo_bin("sd2").unwrap();
    let output = cmd.arg("apply")
       .arg("--manifest")
       .arg(manifest_path.to_str().unwrap())
       .arg("--format=json")
       .output()
       .unwrap();

    if !output.status.success() {
        eprintln!("SD2 Failed: {}", String::from_utf8_lossy(&output.stderr));
        panic!("Command failed");
    }

    let stdout = String::from_utf8(output.stdout).unwrap();
    let events: Vec<serde_json::Value> = stdout.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    
    if events.is_empty() {
        eprintln!("No events found. Stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    let run_start = &events[0]["run_start"];
    // Check if run_start event has transaction_mode "file"
    // Based on json_events_tests.rs, run_start should be first.
    // Assuming structure: { "type": "run_start", "data": { ..., "transaction_mode": "file" } }
    
    // Actually, I need to check the schema of run_start in `src/events.rs`. 
    // It has `transaction_mode: String`.
    
    // In `engine.rs` or `reporter.rs`, run_start is emitted.
    // `src/reporter.rs`: 
    // `transaction_mode: format!("{:?}", pipeline.transaction).to_lowercase()`
    // Transaction::File -> "file"
    
    assert_eq!(run_start["transaction_mode"], "file");

    // Run WITH CLI override "--transaction all"
    let mut cmd = Command::cargo_bin("sd2").unwrap();
    let output = cmd.arg("apply")
       .arg("--manifest")
       .arg(manifest_path.to_str().unwrap())
       .arg("--transaction=all") // Override
       .arg("--format=json")
       .output()
       .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let events: Vec<serde_json::Value> = stdout.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
        
    let run_start = &events[0]["run_start"];
    assert_eq!(run_start["transaction_mode"], "all");
}
