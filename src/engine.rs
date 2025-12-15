use crate::error::{Error, Result};
use crate::model::{Pipeline, Operation};
use crate::replacer::Replacer;
use crate::write::{write_file, WriteOptions};
use crate::reporter::{Report, FileResult};
use crate::input::InputItem;
use crate::model::ReplacementRange;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::PathBuf;
use globset::{Glob, GlobSetBuilder};

/// Execute a pipeline and produce a report.
pub fn execute(mut pipeline: Pipeline, inputs: Vec<InputItem>) -> Result<Report> {
    // Filter inputs based on glob_include and glob_exclude
    let inputs = filter_inputs(inputs, &pipeline.glob_include, &pipeline.glob_exclude)?;

    // validate semantic constraints
    if inputs.is_empty() {
         return Err(Error::Validation("No input sources specified (or all filtered out)".into()));
    }
    if pipeline.operations.is_empty() {
        return Err(Error::Validation("No operations specified".into()));
    }

    let validate_only = pipeline.validate_only;
    // If validate_only is set, force dry_run to true
    if validate_only {
        pipeline.dry_run = true;
    }

    let mut report = Report::new(pipeline.dry_run, validate_only);

    for input in inputs {
        match input {
            InputItem::Path(path_buf) => {
                let path_str = path_buf.to_string_lossy().into_owned();
                let result = process_file(&path_str, &pipeline.operations, &pipeline, None);
                let has_error = result.error.is_some();
                report.add_result(result);

                if has_error {
                    break;
                }
            }
            InputItem::RipgrepMatch { path, matches } => {
                let path_str = path.to_string_lossy().into_owned();
                let result = process_file(&path_str, &pipeline.operations, &pipeline, Some(&matches));
                let has_error = result.error.is_some();
                report.add_result(result);

                if has_error {
                    break;
                }
            }
            InputItem::StdinText(text) => {
                 let result = process_text(text, &pipeline.operations, &pipeline);
                 let has_error = result.error.is_some();
                 report.add_result(result);
                 
                 if has_error {
                    break;
                }
            }
        }
    }

    Ok(report)
}

fn filter_inputs(
    inputs: Vec<InputItem>,
    include: &Option<Vec<String>>,
    exclude: &Option<Vec<String>>,
) -> Result<Vec<InputItem>> {
    if include.is_none() && exclude.is_none() {
        return Ok(inputs);
    }

    let include_set = if let Some(pats) = include {
        let mut b = GlobSetBuilder::new();
        for p in pats {
            b.add(Glob::new(p).map_err(|e| Error::Validation(format!("Invalid glob '{}': {}", p, e)))?);
        }
        Some(b.build().map_err(|e| Error::Validation(format!("Failed to build glob set: {}", e)))?)
    } else {
        None
    };

    let exclude_set = if let Some(pats) = exclude {
        let mut b = GlobSetBuilder::new();
        for p in pats {
             b.add(Glob::new(p).map_err(|e| Error::Validation(format!("Invalid glob '{}': {}", p, e)))?);
        }
        Some(b.build().map_err(|e| Error::Validation(format!("Failed to build glob set: {}", e)))?)
    } else {
        None
    };

    let mut filtered = Vec::new();
    for input in inputs {
        let path = match input {
            InputItem::Path(ref p) => Some(p),
            InputItem::RipgrepMatch { ref path, .. } => Some(path),
            InputItem::StdinText(_) => None,
        };

        if let Some(p) = path {
            // Include logic: If include globs exist, must match at least one.
            if let Some(ref set) = include_set {
                if !set.is_match(p) {
                        continue;
                }
            }
            
            // Exclude logic: If exclude globs exist, must NOT match any.
            if let Some(ref set) = exclude_set {
                if set.is_match(p) {
                    continue;
                }
            }
            
            filtered.push(input);
        } else {
            // Always include stdin text
            filtered.push(input);
        }
    }
    Ok(filtered)
}

fn process_text(
    original: String,
    operations: &[Operation],
    pipeline: &Pipeline,
) -> FileResult {
    // For stdin text, we use a dummy path or "<stdin>"
    let path_buf = PathBuf::from("<stdin>");
    
    match process_content_inner(original.clone(), operations, pipeline, None) {
        Ok((modified, replacements, diff, new_content)) => {
            // If not dry run (and not validate only), we print the new content to stdout
            if !pipeline.dry_run && modified {
                print!("{}", new_content);
            }
            // If unmodified, maybe print original? 
            // The spec says: "returns counts/diff as stdout content ... output goes to stdout"
            // If it's a filter, it should output content. 
            // If no changes, it should output original content.
            if !pipeline.dry_run && !modified {
                print!("{}", original);
            }

            FileResult {
                path: path_buf,
                modified,
                replacements,
                error: None,
                diff,
            }
        },
        Err(e) => FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            diff: None,
        },
    }
}

/// Process a single file.
fn process_file(
    path: &str,
    operations: &[Operation],
    pipeline: &Pipeline,
    matches: Option<&[ReplacementRange]>,
) -> FileResult {
    let path_buf = PathBuf::from(path);
    
    // Read file content
    let content_bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => return FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            diff: None,
        }
    };
    
    let original = String::from_utf8_lossy(&content_bytes).to_string();

    match process_content_inner(original, operations, pipeline, matches) {
        Ok((modified, replacements, diff, new_content)) => {
            // Write changes if modified and not dry_run
            if modified && !pipeline.dry_run {
                let options = WriteOptions {
                    // This will be replaced by new symlink/binary/permissions logic
                    // Currently no_follow_symlinks is the only field.
                    // This is temporary until write::write_file is updated.
                    no_follow_symlinks: pipeline.symlinks != crate::model::Symlinks::Follow,
                };
                if let Err(e) = write_file(&path_buf, new_content.as_bytes(), &options) {
                     return FileResult {
                        path: path_buf,
                        modified: false,
                        replacements: 0,
                        error: Some(e.to_string()),
                        diff: None,
                    };
                }
            }

            FileResult {
                path: path_buf,
                modified,
                replacements,
                error: None,
                diff,
            }
        },
        Err(e) => FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            diff: None,
        },
    }
}

/// Inner processing logic shared between file and text input
fn process_content_inner(
    original: String,
    operations: &[Operation],
    pipeline: &Pipeline,
    matches: Option<&[ReplacementRange]>,
) -> Result<(bool, usize, Option<String>, String)> {
    
    // Apply each operation sequentially
    let mut current = original.clone();
    let mut total_replacements = 0;

    for op in operations {
        match op {
            Operation::Replace { find, with: replacement, literal, ignore_case, smart_case,
                word, multiline, dot_matches_newline, no_unicode, limit, range } => {
                // Build replacer
                let replacer = Replacer::new(
                    find,
                    replacement,
                    *literal,
                    *ignore_case,
                    *smart_case,
                    !(*ignore_case || *smart_case), // case_sensitive
                    *word,
                    *multiline,
                    false, // single_line (not yet supported)
                    *dot_matches_newline,
                    *no_unicode,
                    false, // crlf
                    *limit,
                    range.clone(),
                    matches.map(|m| m.to_vec()),
                ).map_err(|e| Error::Validation(e.to_string()))?;

                // Apply replacement to current string (as bytes) and count replacements
                let (bytes, replacements) = replacer.replace_with_count(current.as_bytes());
                let new_string = String::from_utf8(bytes.to_vec())
                    .map_err(|e| Error::Validation(format!("Invalid UTF-8 after replacement: {}", e)))?;

                current = new_string;
                total_replacements += replacements;
            }
        }
    }

    let modified = current != original;
    let diff = if pipeline.dry_run {
        generate_diff(&original, &current)
    } else {
        None
    };

    Ok((modified, total_replacements, diff, current))
}


/// Generate a unified diff between old and new content.
fn generate_diff(old: &str, new: &str) -> Option<String> {
    if old == new {
        return None;
    }
    let diff = TextDiff::from_lines(old, new);
    let mut output = String::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        output.push_str(&format!("{}{}", sign, change));
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Pipeline, Operation};

    fn pipeline(dry_run: bool, validate_only: bool) -> Pipeline {
        Pipeline {
            dry_run,
            validate_only,
            ..Default::default()
        }
    }

    #[test]
    fn process_content_inner_replaces_and_counts() {
        let mut p = pipeline(true, false);
        let ops = vec![
            Operation::Replace {
                find: "world".into(),
                with: "there".into(),
                literal: true,
                ignore_case: false,
                smart_case: false,
                word: false,
                multiline: false,
                dot_matches_newline: false,
                no_unicode: false,
                limit: 0, // 0 means unlimited
                range: None,
            },
        ];

        let original = "hello world\n".to_string();
        let (modified, replacements, diff, new_content) =
            process_content_inner(original.clone(), &ops, &p, None).unwrap();

        assert!(modified);
        assert_eq!(replacements, 1);
        assert_eq!(new_content, "hello there\n");
        assert!(diff.is_some());
    }

    #[test]
    fn process_content_inner_no_change_no_diff() {
        let p = pipeline(true, false);
        let ops = vec![
            Operation::Replace {
                find: "zzz".into(),
                with: "yyy".into(),
                literal: true,
                ignore_case: false,
                smart_case: false,
                word: false,
                multiline: false,
                dot_matches_newline: false,
                no_unicode: false,
                limit: 0,
                range: None,
            },
        ];

        let original = "abc\n".to_string();
        let (modified, replacements, diff, new_content) =
            process_content_inner(original.clone(), &ops, &p, None).unwrap();

        assert!(!modified);
        assert_eq!(replacements, 0);
        assert_eq!(new_content, original);
        assert!(diff.is_none());
    }

    #[test]
    fn process_content_inner_diff_only_when_dry_run() {
        let p = pipeline(false, false);
        let ops = vec![
            Operation::Replace {
                find: "a".into(),
                with: "b".into(),
                literal: true,
                ignore_case: false,
                smart_case: false,
                word: false,
                multiline: false,
                dot_matches_newline: false,
                no_unicode: false,
                limit: 0,
                range: None,
            },
        ];

        let original = "a\n".to_string();
        let (_modified, _replacements, diff, _new_content) =
            process_content_inner(original, &ops, &p, None).unwrap();

        assert!(diff.is_none());
    }

    #[test]
    fn generate_diff_returns_none_when_equal() {
        assert_eq!(generate_diff("x\n", "x\n"), None);
    }

    #[test]
    fn generate_diff_shows_insert_and_delete_markers() {
        let d = generate_diff("a\n", "b\n").unwrap();
        assert!(d.contains("-a"));
        assert!(d.contains("+b"));
    }

    #[test]
    fn filter_inputs_include_exclude_paths() {
        let inputs = vec![
            InputItem::Path(PathBuf::from("src/main.rs")),
            InputItem::Path(PathBuf::from("src/lib.rs")),
            InputItem::Path(PathBuf::from("README.md")),
            InputItem::StdinText("hi".into()),
        ];

        let include = Some(vec!["src/*.rs".into()]);
        let exclude = Some(vec!["*lib.rs".into()]);

        let out = filter_inputs(inputs, &include, &exclude).unwrap();

        // should include src/main.rs and stdin, exclude src/lib.rs, exclude README.md by include rule
        assert_eq!(out.len(), 2);

        let mut got_main = false;
        let mut got_stdin = false;

        for it in out {
            match it {
                InputItem::Path(p) => {
                    if p == PathBuf::from("src/main.rs") {
                        got_main = true;
                    }
                }
                InputItem::StdinText(_) => got_stdin = true,
                _ => {} // Ignore other variants for this test
            }
        }

        assert!(got_main);
        assert!(got_stdin);
    }

    #[test]
    fn filter_inputs_invalid_glob_is_validation_error() {
        let inputs = vec![InputItem::Path(PathBuf::from("src/main.rs"))];
        let include = Some(vec!["[".into()]); // invalid glob
        let err = filter_inputs(inputs, &include, &None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Invalid glob"));
    }

    #[test]
    fn execute_errors_when_no_inputs() {
        let p = pipeline(true, false);
        let err = execute(p, vec![]).unwrap_err();
        assert!(err.to_string().contains("No input sources specified"));
    }

    #[test]
    fn execute_errors_when_no_operations() {
        let p = pipeline(true, false);
        let err = execute(p, vec![InputItem::StdinText("x".into())]).unwrap_err();
        assert!(err.to_string().contains("No operations specified"));
    }

    #[test]
    fn execute_validate_only_forces_dry_run_and_generates_diff() {
        let mut p = pipeline(false, true);
        p.operations = vec![Operation::Replace {
            find: "a".into(),
            with: "b".into(),
            literal: true,
            ignore_case: false,
            smart_case: false,
            word: false,
            multiline: false,
            dot_matches_newline: false,
            no_unicode: false,
            limit: 0,
            range: None,
        }];

        let report = execute(p, vec![InputItem::StdinText("a\n".into())]).unwrap();

        // Check report.results via inspection or public API
        // Here we just check one result exists
        assert!(!report.files.is_empty());
        let res = &report.files[0];
        assert!(res.diff.is_some());
    }
}
