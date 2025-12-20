use crate::error::{Error, Result};
use crate::model::{Pipeline, Operation, Transaction, Symlinks, BinaryFileMode};
use crate::replacer::Replacer;
use crate::write::{write_file, stage_file, WriteOptions, StagedEntry};
use crate::reporter::{Report, FileResult};
use crate::input::InputItem;
use crate::model::ReplacementRange;
use crate::transaction::TransactionManager;
use crate::policy::{PolicyEnforcer, enforce_pre_execution};
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::{Path, PathBuf, Component};
use std::env;
use std::time::Instant;
use globset::{Glob, GlobSet, GlobSetBuilder};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Execute a pipeline and produce a report.
pub fn execute(mut pipeline: Pipeline, inputs: Vec<InputItem>) -> Result<Report> {
    let start_time = Instant::now();

    // validate semantic constraints
    if inputs.is_empty() {
         return Err(Error::Validation("No input sources specified".into()));
    }
    if pipeline.operations.is_empty() {
        return Err(Error::Validation("No operations specified".into()));
    }

    // Build glob sets
    let (include_set, exclude_set) = build_glob_sets(&pipeline.glob_include, &pipeline.glob_exclude)?;

    enforce_pre_execution(&mut pipeline);
    let enforcer = PolicyEnforcer::new(&pipeline);

    let mut report = Report::new(pipeline.dry_run, pipeline.validate_only);

    let mut tm = if pipeline.transaction == Transaction::All {
        Some(TransactionManager::new())
    } else {
        None
    };

    let cwd = env::current_dir().map_err(|e| Error::Validation(format!("Failed to get current directory: {}", e)))?;


    // Define the processing function (closure)
    let process_item = |input: InputItem| -> (FileResult, Option<StagedEntry>) {
        // Check globs first
        let path_for_glob = match &input {
            InputItem::Path(p) => Some(p.as_path()),
            InputItem::RipgrepMatch { path, .. } => Some(path.as_path()),
            InputItem::StdinText(_) => None,
        };

        if let Some(p) = path_for_glob {
             let normalized = normalize_path(p, &cwd);
             if let Some(ref set) = include_set {
                if !set.is_match(&normalized) {
                    // Report skipped (glob include mismatch)
                     return (FileResult {
                        path: p.to_path_buf(),
                        modified: false,
                        replacements: 0,
                        error: None,
                        error_code: None,
                        skipped: Some("glob exclude".into()), // "glob exclude" covers "not in include"
                        diff: None,
                        diff_is_binary: false,
                        generated_content: None,
                        is_virtual: false,
                    }, None);
                }
             }
             if let Some(ref set) = exclude_set {
                 if set.is_match(&normalized) {
                     // Report skipped (glob exclude)
                      return (FileResult {
                        path: p.to_path_buf(),
                        modified: false,
                        replacements: 0,
                        error: None,
                        error_code: None,
                        skipped: Some("glob exclude".into()),
                        diff: None,
                        diff_is_binary: false,
                        generated_content: None,
                        is_virtual: false,
                    }, None);
                 }
             }
        }

        match input {
            InputItem::Path(path_buf) => {
                let path_str = path_buf.to_string_lossy().into_owned();
                process_file(&path_str, &pipeline.operations, &pipeline, None, &enforcer)
            }
            InputItem::RipgrepMatch { path, matches } => {
                let path_str = path.to_string_lossy().into_owned();
                process_file(&path_str, &pipeline.operations, &pipeline, Some(&matches), &enforcer)
            }
            InputItem::StdinText(text) => {
                 let result = process_text(text, &pipeline.operations, &pipeline);
                 (result, None)
            }
        }
    };

    // Execute in parallel or serial
    #[cfg(feature = "parallel")]
    let results: Vec<(FileResult, Option<StagedEntry>)> = inputs.into_par_iter().map(process_item).collect();

    #[cfg(not(feature = "parallel"))]
    let results: Vec<(FileResult, Option<StagedEntry>)> = inputs.into_iter().map(process_item).collect();

    // Aggregate results
    for (result, staged) in results {
        let has_error = result.error.is_some();
        report.add_result(result);

        if let Some(s) = staged {
            if let Some(manager) = &mut tm {
                manager.stage(s);
            }
        }

        if has_error {
            break;
        }
    }

    // Policy checks
    enforcer.enforce_post_run(&mut report);

    // Commit if no errors and no policy violations
    if enforcer.should_commit(&report) {
        if let Some(manager) = tm {
            manager.commit().map_err(|e| Error::TransactionFailure(e.to_string()))?;
        }
        report.committed = !pipeline.dry_run; // Only true if not dry-run
    } else {
        report.committed = false;
    }

    report.duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(report)
}

fn build_glob_sets(
    include: &Option<Vec<String>>,
    exclude: &Option<Vec<String>>,
) -> Result<(Option<GlobSet>, Option<GlobSet>)> {
    if include.is_none() && exclude.is_none() {
        return Ok((None, None));
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

    Ok((include_set, exclude_set))
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
            let generated_content = if !pipeline.dry_run {
                if modified {
                    Some(new_content)
                } else {
                    Some(original)
                }
            } else {
                None
            };

            FileResult {
                path: path_buf,
                modified,
                replacements,
                error: None,
                error_code: None,
                skipped: None,
                diff,
                diff_is_binary: false, // Text input is always treated as text
                generated_content,
                is_virtual: true,
            }
        },
        Err(e) => FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            error_code: Some(e.code().into()),
            skipped: None,
            diff: None,
            diff_is_binary: false,
            generated_content: None,
            is_virtual: true,
        },
    }
}

/// Process a single file.
fn process_file(
    path: &str,
    operations: &[Operation],
    pipeline: &Pipeline,
    matches: Option<&[ReplacementRange]>,
    enforcer: &PolicyEnforcer,
) -> (FileResult, Option<StagedEntry>) {
    let path_buf = PathBuf::from(path);

    // Check for symlinks
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.is_symlink() {
            match pipeline.symlinks {
                Symlinks::Follow => {
                    // Continue to read
                }
                Symlinks::Skip => {
                    return (FileResult {
                        path: path_buf,
                        modified: false,
                        replacements: 0,
                        error: None,
                        error_code: None,
                        skipped: Some("symlink".into()),
                        diff: None,
                        diff_is_binary: false,
                        generated_content: None,
                        is_virtual: false,
                    }, None);
                }
                Symlinks::Error => {
                    return (FileResult {
                        path: path_buf,
                        modified: false,
                        replacements: 0,
                        error: Some("Encountered symlink with --symlinks error".into()),
                        error_code: Some("E_SYMLINK".into()),
                        skipped: None,
                        diff: None,
                        diff_is_binary: false,
                        generated_content: None,
                        is_virtual: false,
                    }, None);
                }
            }
        }
    }
    
    // Read file content
    let content_bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => return (FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            error_code: Some(match e.kind() {
                std::io::ErrorKind::NotFound => "E_NOT_FOUND".into(),
                std::io::ErrorKind::PermissionDenied => "E_ACCES".into(),
                _ => "E_IO".into(),
            }),
            skipped: None,
            diff: None,
            diff_is_binary: false,
            generated_content: None,
            is_virtual: false,
        }, None)
    };

    // Check for binary content
    if content_bytes.contains(&0) {
        match pipeline.binary {
            BinaryFileMode::Skip => {
                 return (FileResult {
                    path: path_buf,
                    modified: false,
                    replacements: 0,
                    error: None,
                    error_code: None,
                    skipped: Some("binary file".into()),
                    diff: None,
                    diff_is_binary: true,
                    generated_content: None,
                    is_virtual: false,
                }, None);
            }
            BinaryFileMode::Error => {
                return (FileResult {
                    path: path_buf,
                    modified: false,
                    replacements: 0,
                    error: Some("Binary file detected".into()),
                    error_code: Some("E_BINARY".into()),
                    skipped: None,
                    diff: None,
                    diff_is_binary: true,
                    generated_content: None,
                    is_virtual: false,
                }, None);
            }
        }
    }
    
    let original = String::from_utf8_lossy(&content_bytes).to_string();

    match process_content_inner(original, operations, pipeline, matches) {
        Ok((modified, replacements, diff, new_content)) => {
            // Write changes if policy allows
            if enforcer.can_write(modified) {
                let options = WriteOptions {
                    no_follow_symlinks: pipeline.symlinks != crate::model::Symlinks::Follow,
                    permissions: pipeline.permissions.clone(),
                };
                
                if enforcer.should_stage() {
                    // Stage
                    match stage_file(&path_buf, new_content.as_bytes(), &options) {
                        Ok(staged) => (FileResult {
                            path: path_buf,
                            modified,
                            replacements,
                            error: None,
                            error_code: None,
                            skipped: None,
                            diff,
                            diff_is_binary: false,
                            generated_content: None,
                            is_virtual: false,
                        }, Some(staged)),
                        Err(e) => (FileResult {
                            path: path_buf,
                            modified: false,
                            replacements: 0,
                            error: Some(e.to_string()),
                            error_code: Some(e.code().into()),
                            skipped: None,
                            diff: None,
                            diff_is_binary: false,
                            generated_content: None,
                            is_virtual: false,
                        }, None),
                    }
                } else {
                    // Write immediately
                    if let Err(e) = write_file(&path_buf, new_content.as_bytes(), &options) {
                         return (FileResult {
                            path: path_buf,
                            modified: false,
                            replacements: 0,
                            error: Some(e.to_string()),
                            error_code: Some(e.code().into()),
                            skipped: None,
                            diff: None,
                            diff_is_binary: false,
                            generated_content: None,
                            is_virtual: false,
                        }, None);
                    }
                    
                    (FileResult {
                        path: path_buf,
                        modified,
                        replacements,
                        error: None,
                        error_code: None,
                        skipped: None,
                        diff,
                        diff_is_binary: false,
                        generated_content: None,
                        is_virtual: false,
                    }, None)
                }
            } else {
                 (FileResult {
                    path: path_buf,
                    modified,
                    replacements,
                    error: None,
                    error_code: None,
                    skipped: None,
                    diff,
                    diff_is_binary: false,
                    generated_content: None,
                    is_virtual: false,
                }, None)
            }
        },
        Err(e) => (FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            error_code: Some(e.code().into()),
            skipped: None,
            diff: None,
            diff_is_binary: false,
            generated_content: None,
            is_virtual: false,
        }, None),
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
                word, multiline, dot_matches_newline, no_unicode, limit, range, expand, validation_mode } => {
                // Build replacer
                let replacer = Replacer::new(
                    find,
                    replacement,
                    *literal,
                    *ignore_case,
                    *smart_case,
                    *word,
                    *multiline,
                    false, // single_line (not yet supported)
                    *dot_matches_newline,
                    *no_unicode,
                    *limit,
                    range.clone(),
                    matches.map(|m| m.to_vec()),
                    *expand,
                    *validation_mode,
                ).map_err(|e| Error::Validation(e.to_string()))?;

                // Apply replacement to current string (as bytes) and count replacements
                let (bytes, replacements) = replacer.replace_with_count(current.as_bytes());
                let new_string = String::from_utf8(bytes.to_vec())
                    .map_err(|e| Error::Validation(format!("Invalid UTF-8 after replacement: {}", e)))?;

                current = new_string;
                total_replacements += replacements;
            }
            Operation::Delete { find, literal, ignore_case, smart_case,
                word, multiline, dot_matches_newline, no_unicode, limit, range } => {
                // Build replacer with empty replacement
                let replacer = Replacer::new(
                    find,
                    "", // empty replacement
                    *literal,
                    *ignore_case,
                    *smart_case,
                    *word,
                    *multiline,
                    false, // single_line
                    *dot_matches_newline,
                    *no_unicode,
                    *limit,
                    range.clone(),
                    matches.map(|m| m.to_vec()),
                    false, // expand (no need for empty string)
                    crate::model::ValidationMode::default(),
                ).map_err(|e| Error::Validation(e.to_string()))?;

                let (bytes, replacements) = replacer.replace_with_count(current.as_bytes());
                let new_string = String::from_utf8(bytes.to_vec())
                    .map_err(|e| Error::Validation(format!("Invalid UTF-8 after delete: {}", e)))?;

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
        // Sanitize: replace any non-UTF8 or weird control chars if they slipped through?
        // Actually, inputs are already Strings (valid UTF-8), but might contain control chars.
        // The requirement is: "force-sanitize invalid UTF-8 in the `diff` string to the replacement character"
        // Since input is `&str`, it is already valid UTF-8. 
        // However, if we want to be extra safe against JSON serialization issues with control chars (though serde handles most):
        // But the prompt says "If a file is technically text but contains a rogue null byte or invalid UTF-8 sequence that your diff engine tries to include".
        // We already converted bytes to String using `String::from_utf8_lossy` earlier in `process_file`, so we shouldn't have invalid UTF-8 here.
        // `from_utf8_lossy` replaces invalid sequences with .
        // So `old` and `new` are safe.
        // But let's just make sure we don't return something that breaks.
        
        output.push_str(&format!("{}{}", sign, change));
        if change.missing_newline() {
            output.push_str("\n\\ No newline at end of file\n");
        }
    }
    Some(output)
}

fn normalize_path(path: &Path, cwd: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.strip_prefix(cwd).unwrap_or(path)
    } else {
        path
    };

    let components = path.components();
    let mut filtered = PathBuf::new();
    let mut pushed = false;
    for component in components {
        if component == Component::CurDir {
            continue;
        }
        filtered.push(component);
        pushed = true;
    }
    
    if !pushed {
        return PathBuf::from(".");
    }
    filtered
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

    fn op_replace(find: &str, with: &str) -> Operation {
        Operation::Replace {
            find: find.into(),
            with: with.into(),
            literal: true,
            ignore_case: false,
            smart_case: false,
            word: false,
            multiline: false,
            dot_matches_newline: false,
            no_unicode: false,
            limit: 0,
            range: None,
            expand: false,
            validation_mode: crate::model::ValidationMode::default(),
        }
    }

    #[test]
    fn process_content_inner_replaces_and_counts() {
        let p = pipeline(true, false);
        let ops = vec![op_replace("world", "there")];

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
        let ops = vec![op_replace("zzz", "yyy")];

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
        let ops = vec![op_replace("a", "b")];

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
    fn build_glob_sets_valid() {
        let include = Some(vec!["src/*.rs".into()]);
        let exclude = Some(vec!["*lib.rs".into()]);
        let (inc, exc) = build_glob_sets(&include, &exclude).unwrap();
        assert!(inc.is_some());
        assert!(exc.is_some());
    }

    #[test]
    fn build_glob_sets_invalid() {
        let include = Some(vec!["[".into()]);
        let err = build_glob_sets(&include, &None).unwrap_err();
        assert!(err.to_string().contains("Invalid glob"));
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
        p.operations = vec![op_replace("a", "b")];

        let report = execute(p, vec![InputItem::StdinText("a\n".into())]).unwrap();

        // Check report.results via inspection or public API
        // Here we just check one result exists
        assert!(!report.files.is_empty());
        let res = &report.files[0];
        assert!(res.diff.is_some());
    }

    // Policy tests
    #[test]
    fn execute_require_match_fails_if_no_match() {
        let mut p = pipeline(true, false);
        p.require_match = true;
        p.operations = vec![op_replace("foo", "bar")];
        
        let report = execute(p, vec![InputItem::StdinText("baz".into())]).unwrap();
        
        assert!(report.policy_violation.is_some());
        assert!(report.policy_violation.as_ref().unwrap().contains("No matches found"));
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn execute_expect_n_fails_if_count_mismatch() {
        let mut p = pipeline(true, false);
        p.expect = Some(2);
        p.operations = vec![op_replace("foo", "bar")];
        
        // Only 1 match
        let report = execute(p, vec![InputItem::StdinText("foo".into())]).unwrap();
        
        assert!(report.policy_violation.is_some());
        assert!(report.policy_violation.as_ref().unwrap().contains("Expected 2 replacements, found 1"));
        assert_eq!(report.exit_code(), 2);
    }

        #[test]
        fn execute_fail_on_change_fails_if_modified() {
            let mut p = pipeline(true, false); // dry_run
            p.fail_on_change = true;
            p.operations = vec![op_replace("foo", "bar")];
            
            let report = execute(p, vec![InputItem::StdinText("foo".into())]).unwrap();
            
            assert!(report.modified > 0);
            assert!(report.policy_violation.is_some());
            assert!(report.policy_violation.as_ref().unwrap().contains("Changes detected"));
            assert_eq!(report.exit_code(), 2);
        }
    }
