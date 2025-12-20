use assert_cmd::cargo::cargo_bin_cmd;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn rg_json_span_targeting() {
    // 1. Create a file with multiple occurrences of "foo"
    //    foo at line 1 (offset 0..3)
    //    foo at line 2 (offset 4..7)
    //    foo at line 3 (offset 8..11)
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "foo").unwrap();
    writeln!(file, "foo").unwrap();
    writeln!(file, "foo").unwrap();
    let path = file.path().to_str().unwrap().to_string();

    // 2. Create rg JSON output that only targets the MIDDLE "foo" (line 2)
    //    Offset of line 2 is 4. "foo" is at 4..7.
    //    Rg submatch relative to line: start 0, end 3.
    //    So absolute range: 4..7.
    let rg_json = format!(
        r#"{{"type":"match","data":{{"path":{{"text":"{}"}},"lines":{{"text":"foo\n"}},"line_number":2,"absolute_offset":4,"submatches":[{{"match":{{"text":"foo"}},"start":0,"end":3}}]}}}}"#,
        path.replace("\\", "\\\\") // Handle windows paths if needed, though linux is assumed here
    );

    // 3. Run stedi with --rg-json, replacing "foo" with "bar"
    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--rg-json")
        .write_stdin(rg_json)
        .assert()
        .success();

    // 4. Verify content:
    //    line 1: foo (untouched)
    //    line 2: bar (replaced)
    //    line 3: foo (untouched)
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "foo\nbar\nfoo\n");
}

#[test]
fn rg_json_multiple_submatches() {
    // 1. File with multiple matches on one line
    //    "foo foo"
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "foo foo").unwrap();
    let path = file.path().to_str().unwrap().to_string();

    // 2. Rg JSON targeting only the SECOND "foo"
    //    Line offset 0.
    //    First foo: 0..3
    //    Space: 3..4
    //    Second foo: 4..7
    //    Rg submatch for second foo: start 4, end 7.
    let rg_json = format!(
        r#"{{"type":"match","data":{{"path":{{"text":"{}"}},"lines":{{"text":"foo foo\n"}},"line_number":1,"absolute_offset":0,"submatches":[{{"match":{{"text":"foo"}},"start":4,"end":7}}]}}}}"#,
        path.replace("\\", "\\\\")
    );

    let mut cmd = cargo_bin_cmd!("stedi");
    cmd.arg("foo")
        .arg("bar")
        .arg("--rg-json")
        .write_stdin(rg_json)
        .assert()
        .success();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "foo bar\n");
}
