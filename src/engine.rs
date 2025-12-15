use crate::error::{Error, Result};
use crate::model::{Pipeline, Operation};
use crate::replacer::Replacer;
use crate::write::{write_file, WriteOptions};
use crate::reporter::{Report, FileResult};
use crate::input::InputItem;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::PathBuf;

/// Execute a pipeline and produce a report.
pub fn execute(mut pipeline: Pipeline, inputs: Vec<InputItem>) -> Result<Report> {
    // validate semantic constraints
    if inputs.is_empty() {
         return Err(Error::Validation("No input sources specified".into()));
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
                let result = process_file(&path_str, &pipeline.operations, &pipeline);
                let has_error = result.error.is_some();
                report.add_result(result);

                if !pipeline.continue_on_error && has_error {
                    break;
                }
            }
            InputItem::StdinText(text) => {
                 let result = process_text(text, &pipeline.operations, &pipeline);
                 let has_error = result.error.is_some();
                 report.add_result(result);
                 
                 if !pipeline.continue_on_error && has_error {
                    break;
                }
            }
        }
    }

    Ok(report)
}

fn process_text(
    original: String,
    operations: &[Operation],
    pipeline: &Pipeline,
) -> FileResult {
    // For stdin text, we use a dummy path or "<stdin>"
    let path_buf = PathBuf::from("<stdin>");
    
    match process_content_inner(original.clone(), operations, pipeline) {
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

    match process_content_inner(original, operations, pipeline) {
        Ok((modified, replacements, diff, new_content)) => {
            // Write changes if modified and not dry_run
            if modified && !pipeline.dry_run {
                let options = WriteOptions {
                    backup: if pipeline.backup {
                        Some(pipeline.backup_ext.clone())
                    } else {
                        None
                    },
                    no_follow_symlinks: !pipeline.follow_symlinks,
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
    let diff = if pipeline.dry_run || pipeline.backup {
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