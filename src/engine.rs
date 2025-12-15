use crate::error::{Error, Result};
use crate::model::{Pipeline, Operation};
use crate::replacer::Replacer;
use crate::write::{write_file, WriteOptions};
use crate::reporter::{Report, FileResult};
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::{Path, PathBuf};

/// Execute a pipeline and produce a report.
pub fn execute(mut pipeline: Pipeline) -> Result<Report> {
    // validate semantic constraints
    if pipeline.files.is_empty() {
         return Err(Error::Validation("No files specified for processing".into()));
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

    for file_path in &pipeline.files {
        let result = process_file(&file_path, &pipeline.operations, &pipeline);
        let has_error = result.error.is_some();
        report.add_result(result);

        // If continue_on_error is false and error occurred, break
        if !pipeline.continue_on_error && has_error {
            break;
        }
    }

    Ok(report)
}

/// Process a single file.
fn process_file(
    path: &str,
    operations: &[Operation],
    pipeline: &Pipeline,
) -> FileResult {
    let path_buf = PathBuf::from(path);
    match process_file_inner(&path_buf, operations, pipeline) {
        Ok((modified, replacements, diff)) => FileResult {
            path: path_buf,
            modified,
            replacements,
            error: None,
            diff,
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

/// Inner processing that can fail.
fn process_file_inner(
    path: &Path,
    operations: &[Operation],
    pipeline: &Pipeline,
) -> Result<(bool, usize, Option<String>)> {
    // Read file content
    let content = fs::read(path)?;
    let original = String::from_utf8_lossy(&content).to_string();

    // Apply each operation sequentially
    let mut current = original.clone();
    let mut total_replacements = 0;

    for op in operations {
        match op {
            Operation::Replace { find, with: replacement, literal, ignore_case, smart_case,
                word, multiline, dot_matches_newline, no_unicode, limit } => {
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

    // Write changes if modified and not dry_run
    if modified && !pipeline.dry_run {
        let options = WriteOptions {
            backup: if pipeline.backup {
                Some(pipeline.backup_ext.clone())
            } else {
                None
            },
            follow_symlinks: pipeline.follow_symlinks,
            no_follow_symlinks: !pipeline.follow_symlinks,
        };
        write_file(path, current.as_bytes(), &options)?;
    }

    // TODO: compute actual replacements count from diff
    Ok((modified, total_replacements, diff))
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