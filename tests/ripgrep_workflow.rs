use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::tempdir;

#[test]
fn test_rg_l_integration() {
    // 1. Setup
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.rs");
    fs::write(&file_path, "fn main() { unwrap(); }").unwrap();

    // 2. Run rg -l to get the file list
    // We assume /usr/bin/rg is available as per prompt
    let rg_output = StdCommand::new("/usr/bin/rg")
        .arg("-l")
        .arg("unwrap")
        .current_dir(dir.path())
        .output()
        .expect("failed to execute rg");

    assert!(rg_output.status.success(), "rg failed");

    // 3. Run sd2 using the output from rg
    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("unwrap()")
       .arg("expect(\"safe\")")
       .write_stdin(rg_output.stdout)
       .current_dir(dir.path()) // Important so sd2 finds the file by relative path if rg output is relative
       .assert()
       .success();

    // 4. Verify content
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "fn main() { expect(\"safe\"); }");
}

#[test]
fn test_rg_json_integration() {
    // 1. Setup
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("messy.rs");
    // Messy content: multiple unwraps, some inside strings (which rg might match depending on pattern, but sd2 replacer logic usually replaces everything unless scoped. 
    // BUT --rg-json mode means sd2 receives specific ranges. 
    // If rg matches "unwrap" inside a string, sd2 will replace it if we pass "unwrap" as FIND.
    // However, the power of --rg-json is we can use rg's smarts (e.g. rust grammar if rg supported it, or just precise context).
    // Let's test that sd2 *only* touches what rg reports.
    
    let content = r#" 
fn main() {
    let x = option.unwrap();
    let y = "don't unwrap me";
    let z = option.unwrap();
}
"#;
    fs::write(&file_path, content).unwrap();

    // 2. Run rg --json
    // We search for "unwrap" but only matching word boundary to avoid "unwrap" in string if possible? 
    // rg doesn't know rust grammar by default. 
    // Let's just search for "unwrap" and see if sd2 replaces ALL of them if rg reports ALL of them.
    // To prove targetedness, we can construct a scenario where rg *skips* one but sd2 would match it if it scanned the file.
    // Example: rg search for "unwrap" but restricted to line 3.
    // rg doesn't easily restrict to lines via CLI args except via -N.
    // Actually, `sd2 --rg-json FIND REPLACE` uses `FIND` to build the `Replacer`.
    // It *also* uses the ranges from rg.
    // So `sd2` will ONLY replace if:
    // 1. The text matches `FIND` (as regex/literal)
    // 2. AND the text is within the ranges reported by rg.
    
    // So if we have "unwrap" twice, and rg only reports one (e.g. because we grepped for `let x.*unwrap`), 
    // sd2 should only replace that one.
    
    let rg_output = StdCommand::new("/usr/bin/rg")
        .arg("--json")
        .arg("let x.*unwrap") // Only matches the first line
        .current_dir(dir.path())
        .output()
        .expect("failed to execute rg");

    assert!(rg_output.status.success());

    // 3. Run sd2 --rg-json
    // We want to replace "unwrap" with "expect"
    // The rg match is "let x = option.unwrap();" (the whole line matches the pattern)
    // The rg json will contain a submatch for the whole match? 
    // Wait, rg json `submatches` usually contains the match of the pattern.
    // If pattern is "let x.*unwrap", the match is that whole string.
    // sd2 receives that range.
    // sd2's FIND is "unwrap".
    // sd2 will search for "unwrap" *within* the range "let x = option.unwrap();".
    // It will find it and replace it.
    // It will NOT replace the "unwrap" on the z line because rg didn't report that line.
    
    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("--rg-json")
       .arg("unwrap")
       .arg("expect")
       .write_stdin(rg_output.stdout)
       .current_dir(dir.path())
       .assert()
       .success();

    // 4. Verify
    let new_content = fs::read_to_string(&file_path).unwrap();
    // Line 3 (x) should change.
    // Line 5 (z) should NOT change, even though it contains "unwrap", because rg didn't match it.
    assert!(new_content.contains("let x = option.expect();"));
    assert!(new_content.contains("let z = option.unwrap();")); 
}

#[test]
fn test_rg_json_messy_utf8() {
    // 1. Setup
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("unicode.rs");
    let content = "fn main() { v.unwrap(); // 🦀 }"; 
    fs::write(&file_path, content).unwrap();

    // 2. Run rg --json
    let rg_output = StdCommand::new("/usr/bin/rg")
        .arg("--json")
        .arg("unwrap")
        .current_dir(dir.path())
        .output()
        .expect("failed to execute rg");

    // 3. Run sd2
    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("--rg-json")
       .arg("unwrap")
       .arg("expect")
       .write_stdin(rg_output.stdout)
       .current_dir(dir.path())
       .assert()
       .success();

    // 4. Verify
    let new_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(new_content, "fn main() { v.expect(); // 🦀 }");
}

#[test]
fn test_real_world_engine_rs() {
    // 1. Setup
    let dir = tempdir().unwrap();
    let engine_src = std::path::Path::new("src/engine.rs");
    if !engine_src.exists() {
        // Fallback for when running in different CWD, though cargo test usually sets CWD to package root
        eprintln!("Skipping real-world test: src/engine.rs not found");
        return;
    }
    let file_path = dir.path().join("engine.rs");
    fs::copy(engine_src, &file_path).unwrap();

    // 2. rg -l workflow: Replace "Pipeline" with "PipeLine"
    let rg_output = StdCommand::new("/usr/bin/rg")
        .arg("-l")
        .arg("Pipeline")
        .current_dir(dir.path())
        .output()
        .expect("failed to execute rg");

    assert!(rg_output.status.success());

    let mut cmd = Command::cargo_bin("sd2").unwrap();
    cmd.arg("Pipeline")
       .arg("PipeLine")
       .write_stdin(rg_output.stdout)
       .current_dir(dir.path())
       .assert()
       .success();

    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("PipeLine"));
    assert!(!content.contains("Pipeline"));

    // 3. rg --json workflow: Replace "InputItem" with "InItem"
    // Targeted replacement
    let rg_json_output = StdCommand::new("/usr/bin/rg")
        .arg("--json")
        .arg("InputItem")
        .current_dir(dir.path())
        .output()
        .expect("failed to execute rg --json");

    assert!(rg_json_output.status.success());

    let mut cmd2 = Command::cargo_bin("sd2").unwrap();
    cmd2.arg("--rg-json")
       .arg("InputItem")
       .arg("InItem")
       .write_stdin(rg_json_output.stdout)
       .current_dir(dir.path())
       .assert()
       .success();

    let content_final = fs::read_to_string(&file_path).unwrap();
    assert!(content_final.contains("use crate::input::InItem;"));
    assert!(content_final.contains("inputs: Vec<InItem>"));
}
