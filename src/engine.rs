use crate::error::{Error, Result};
use crate::model::{Pipeline, Operation, Transaction, Symlinks, BinaryFileMode};
use crate::replacer::Replacer;
use crate::write::{write_file, stage_file, WriteOptions};
use crate::reporter::{Report, FileResult};
use crate::input::InputItem;
use crate::model::ReplacementRange;
use crate::transaction::TransactionManager;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::{Path, PathBuf, Component};
use std::env;
use globset::{Glob, GlobSet, GlobSetBuilder};

/// Execute a pipeline and produce a report.
pub fn execute(mut pipeline: Pipeline, inputs: Vec<InputItem>) -> Result<Report> {
    // validate semantic constraints
    if inputs.is_empty() {
         return Err(Error::Validation("No input sources specified".into()));
    }
    if pipeline.operations.is_empty() {
        return Err(Error::Validation("No operations specified".into()));
    }

    // Build glob sets
    let (include_set, exclude_set) = build_glob_sets(&pipeline.glob_include, &pipeline.glob_exclude)?;

    let validate_only = pipeline.validate_only;
    // If validate_only is set, force dry_run to true
    if validate_only {
        pipeline.dry_run = true;
    }

    let mut report = Report::new(pipeline.dry_run, validate_only);

    let mut tm = if pipeline.transaction == Transaction::All {
        Some(TransactionManager::new())
    } else {
        None
    };

    let cwd = env::current_dir().map_err(|e| Error::Validation(format!("Failed to get current directory: {}", e)))?;

    for input in inputs {
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
                     report.add_result(FileResult {
                        path: p.to_path_buf(),
                        modified: false,
                        replacements: 0,
                        error: None,
                        skipped: Some("glob exclude".into()), // "glob exclude" covers "not in include"
                        diff: None,
                    });
                    continue;
                }
             }
             if let Some(ref set) = exclude_set {
                 if set.is_match(&normalized) {
                     // Report skipped (glob exclude)
                      report.add_result(FileResult {
                        path: p.to_path_buf(),
                        modified: false,
                        replacements: 0,
                        error: None,
                        skipped: Some("glob exclude".into()),
                        diff: None,
                    });
                     continue;
                 }
             }
        }

        match input {
            InputItem::Path(path_buf) => {
                let path_str = path_buf.to_string_lossy().into_owned();
                let result = process_file(&path_str, &pipeline.operations, &pipeline, None, &mut tm);
                let has_error = result.error.is_some();
                report.add_result(result);

                if has_error {
                    break;
                }
            }
            InputItem::RipgrepMatch { path, matches } => {
                let path_str = path.to_string_lossy().into_owned();
                let result = process_file(&path_str, &pipeline.operations, &pipeline, Some(&matches), &mut tm);
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

    // Policy checks
    if pipeline.require_match && report.replacements == 0 {
        report.policy_violation = Some("No matches found (--require-match)".into());
    } else if let Some(expected) = pipeline.expect {
        if report.replacements != expected {
            report.policy_violation = Some(format!(
                "Expected {} replacements, found {} (--expect)",
                expected, report.replacements
            ));
        }
    } else if pipeline.fail_on_change && report.modified > 0 {
        report.policy_violation = Some(format!(
            "Changes detected in {} files (--fail-on-change)",
            report.modified
        ));
    }

    // Commit if no errors and no policy violations
    if report.exit_code() == 0 {
        if let Some(manager) = tm {
            manager.commit().map_err(|e| Error::TransactionFailure(e.to_string()))?;
        }
    }

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
                skipped: None,
                diff,
            }
        },
        Err(e) => FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            skipped: None,
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
    tm: &mut Option<TransactionManager>,
) -> FileResult {
    let path_buf = PathBuf::from(path);

    // Check for symlinks
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.is_symlink() {
            match pipeline.symlinks {
                Symlinks::Follow => {
                    // Continue to read
                }
                Symlinks::Skip => {
                    return FileResult {
                        path: path_buf,
                        modified: false,
                        replacements: 0,
                        error: None,
                        skipped: Some("symlink".into()),
                        diff: None,
                    };
                }
                Symlinks::Error => {
                    return FileResult {
                        path: path_buf,
                        modified: false,
                        replacements: 0,
                        error: Some("Encountered symlink with --symlinks error".into()),
                        skipped: None,
                        diff: None,
                    };
                }
            }
        }
    }
    
    // Read file content
    let content_bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => return FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            skipped: None,
            diff: None,
        }
    };

    // Check for binary content (simple heuristic: look for null byte)
    // We only check the first 8KB to avoid scanning massive files entirely if unnecessary, 
    // although for correctness on the whole file we should scan all. 
    // `grep` usually checks the first few KB.
    // However, if we are about to load it all into a String, we might as well check it all 
    // or rely on `String::from_utf8` failing.
    // But `from_utf8` fails on invalid UTF-8, not necessarily just "binary" (though binary often has invalid utf8).
    // The requirement is specific about 0x00.
    if content_bytes.contains(&0) {
        match pipeline.binary {
            BinaryFileMode::Skip => {
                 return FileResult {
                    path: path_buf,
                    modified: false,
                    replacements: 0,
                    error: None,
                    skipped: Some("binary file".into()),
                    diff: None,
                };
            }
            BinaryFileMode::Error => {
                return FileResult {
                    path: path_buf,
                    modified: false,
                    replacements: 0,
                    error: Some("Binary file detected".into()),
                    skipped: None,
                    diff: None,
                };
            }
        }
    }
    
    let original = String::from_utf8_lossy(&content_bytes).to_string();

    match process_content_inner(original, operations, pipeline, matches) {
        Ok((modified, replacements, diff, new_content)) => {
            // Write changes if modified and not dry_run and not no_write
            if modified && !pipeline.dry_run && !pipeline.no_write {
                let options = WriteOptions {
                    no_follow_symlinks: pipeline.symlinks != crate::model::Symlinks::Follow,
                    permissions: pipeline.permissions.clone(),
                };
                
                if let Some(manager) = tm {
                    // Stage
                    match stage_file(&path_buf, new_content.as_bytes(), &options) {
                        Ok(staged) => manager.stage(staged),
                        Err(e) => return FileResult {
                            path: path_buf,
                            modified: false,
                            replacements: 0,
                            error: Some(e.to_string()),
                            skipped: None,
                            diff: None,
                        },
                    }
                } else {
                    // Write immediately
                    if let Err(e) = write_file(&path_buf, new_content.as_bytes(), &options) {
                         return FileResult {
                            path: path_buf,
                            modified: false,
                            replacements: 0,
                            error: Some(e.to_string()),
                            skipped: None,
                            diff: None,
                        };
                    }
                }
            }

            FileResult {
                path: path_buf,
                modified,
                replacements,
                error: None,
                skipped: None,
                diff,
            }
        },
        Err(e) => FileResult {
            path: path_buf,
            modified: false,
            replacements: 0,
            error: Some(e.to_string()),
            skipped: None,
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

fn normalize_path(path: &Path, cwd: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.strip_prefix(cwd).unwrap_or(path)
    } else {
        path
    };

    let mut components = path.components();
    let mut filtered = PathBuf::new();
    let mut pushed = false;
    while let Some(component) = components.next() {
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