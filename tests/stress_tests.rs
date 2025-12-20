use assert_cmd::cargo::cargo_bin_cmd;
use std::fs::File;
use std::io::{BufWriter, Write};
use tempfile::TempDir;

#[test]
fn test_large_file_performance() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("large_file.txt");

    // Create a 10MB file
    {
        let file = File::create(&file_path).unwrap();
        let mut writer = BufWriter::new(file);
        // Write "hello world\n" repeated many times
        // 12 bytes * 1024 * 1024 / 12 ~= 87381 lines
        let chunk = "hello world\n"; // 12 bytes
        for _ in 0..873_814 {
            writer.write_all(chunk.as_bytes()).unwrap();
        }
        writer.flush().unwrap();
    }

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("world")
        .arg("universe")
        .arg(file_path.to_str().unwrap());

    let start = std::time::Instant::now();
    cmd.assert().success();
    let duration = start.elapsed();

    println!("Processed 10MB file in {:?}", duration);

    // Verify changes
    let content = std::fs::read_to_string(&file_path).unwrap();
    // Check first and last line
    assert!(content.starts_with("hello universe\n"));
    assert!(content.ends_with("hello universe\n"));
    // Rough check of length (universe is 3 bytes longer than world)
    // 10MB + (3 bytes * 873814) ~= 12.6MB
    assert!(content.len() > 12_000_000);
}

#[test]
fn test_many_files_transaction_all() {
    let temp_dir = TempDir::new().unwrap();
    let file_count = 1000;
    let mut file_paths = Vec::new();

    // Create 1000 small files
    for i in 0..file_count {
        let name = format!("file_{}.txt", i);
        let path = temp_dir.path().join(&name);
        std::fs::write(&path, "foo bar baz").unwrap();
        file_paths.push(path);
    }

    // Use files0 mode to avoid command line length limits
    let mut input_data = Vec::new();
    for path in &file_paths {
        input_data.extend_from_slice(path.to_str().unwrap().as_bytes());
        input_data.push(0); // NUL separator
    }

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("bar")
        .arg("qux")
        .arg("--transaction=all") // Test the staging overhead
        .arg("--files0")
        .write_stdin(input_data);

    let start = std::time::Instant::now();
    cmd.assert().success();
    let duration = start.elapsed();

    println!("Processed {} files in {:?}", file_count, duration);

    // Verify a few files
    for i in [0, 500, 999] {
        let content = std::fs::read_to_string(&file_paths[i]).unwrap();
        assert_eq!(content, "foo qux baz");
    }
}

#[test]
fn test_rg_json_streaming() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("data.txt");

    // Create a target file
    let mut content = String::new();
    let lines = 10_000;
    for i in 0..lines {
        content.push_str(&format!("line {}: hello world\n", i));
    }
    std::fs::write(&file_path, &content).unwrap();

    // Create rg-json input targeting "world" on every line
    let mut rg_input = String::new();
    let path_str = file_path.to_str().unwrap();

    // Structure:
    // { "type": "begin", "data": { "path": { "text": "..." } } }
    // { "type": "match", "data": { "path": { "text": "..." }, "lines": { "text": "..." }, "line_number": ..., "absolute_offset": ..., "submatches": [...] } }
    // { "type": "end", "data": { "path": { "text": "..." } } }

    // We only need "match" events for stedi to work?
    // stedi's stream_rg_json_ndjson uses DeinterleavingSink, which likely expects begin/end or at least match events with paths.

    let mut offset = 0;
    for i in 0..lines {
        let line_text = format!("line {}: hello world\n", i);
        // "world" starts at index: "line X: hello ".len()
        let prefix = format!("line {}: hello ", i);
        let match_start = prefix.len();
        let match_end = match_start + 5; // "world"

        let json = serde_json::json!({
            "type": "match",
            "data": {
                "path": { "text": path_str },
                "lines": { "text": line_text.trim_end() },
                "line_number": i + 1,
                "absolute_offset": offset,
                "submatches": [
                    { "match": { "text": "world" }, "start": match_start, "end": match_end }
                ]
            }
        });
        rg_input.push_str(&json.to_string());
        rg_input.push('\n');

        offset += line_text.len();
    }

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("world") // FIND is ignored in rg-json mode usually, or strictly used for replacement?
        // Wait, if using rg-json, we are targeting specific spans.
        // Does stedi require FIND/REPLACE args in rg-json mode?
        // CLI arg "find" is Option<String>.
        // But usually with rg-json, we might be providing replacement via args?
        // Let's check `resolve_input_mode` calls `read_rg_json`.
        // The pipeline still needs operations.
        // The `process_file` logic uses `operations` and applies them.
        // If matches are provided (from rg-json), `Replacer` uses them to restrict where it looks/replaces.
        // BUT `Replacer` still needs a "find" pattern to verify match or just to know what to replace with what?
        // Actually `Replacer::new` takes `find` and `replacement`.
        // If `matches` (ranges) are provided, `Replacer` likely only replaces within those ranges.
        // But it still performs the "find" logic within those ranges?
        // Or does it blindly replace the range with the replacement string?
        // Let's check `replacer/mod.rs` to be sure.
        .arg("universe")
        .arg("--rg-json")
        .write_stdin(rg_input);

    let start = std::time::Instant::now();
    cmd.assert().success();
    let duration = start.elapsed();

    println!("Processed {} matches via rg-json in {:?}", lines, duration);

    let new_content = std::fs::read_to_string(&file_path).unwrap();
    assert!(new_content.contains("line 0: hello universe"));
    assert!(new_content.contains("line 9999: hello universe"));
}
