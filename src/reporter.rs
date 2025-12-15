use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of processing a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    /// Path to the file.
    pub path: PathBuf,
    /// Whether the file was modified.
    pub modified: bool,
    /// Number of replacements performed.
    pub replacements: usize,
    /// Error, if any.
    pub error: Option<String>,
    /// Reason for skipping the file, if skipped.
    pub skipped: Option<String>,
    /// Diff lines (if dry_run or preview).
    pub diff: Option<String>,
}

/// Overall execution report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Results for each file.
    pub files: Vec<FileResult>,
    /// Total files processed.
    pub total: usize,
    /// Total files modified.
    pub modified: usize,
    /// Total replacements performed.
    pub replacements: usize,
    /// Whether dry-run mode was active.
    pub dry_run: bool,
    /// Whether validate-only mode was active.
    pub validate_only: bool,
    /// Whether any errors occurred.
    pub has_errors: bool,
    /// Policy violation message (if any).
    pub policy_violation: Option<String>,
}

impl Report {
    /// Create a new empty report.
    pub fn new(dry_run: bool, validate_only: bool) -> Self {
        Self {
            files: Vec::new(),
            total: 0,
            modified: 0,
            replacements: 0,
            dry_run,
            validate_only,
            has_errors: false,
            policy_violation: None,
        }
    }

    /// Add a file result.
    pub fn add_result(&mut self, result: FileResult) {
        self.total += 1;
        if result.modified {
            self.modified += 1;
        }
        self.replacements += result.replacements;
        if result.error.is_some() {
            self.has_errors = true;
        }
        self.files.push(result);
    }

    /// Print report in human-readable format.
    pub fn print_human(&self) {
        if let Some(msg) = &self.policy_violation {
            eprintln!("Policy Error: {}", msg);
        }

        if self.validate_only {
            println!("VALIDATION RUN - No files were written.");
        } else if self.dry_run {
            println!("DRY RUN - No files were written.");
        }
        println!("Processed {} files, modified {}, {} replacements.",
                 self.total, self.modified, self.replacements);
        for file in &self.files {
            if let Some(err) = &file.error {
                println!("  {}: ERROR - {}", file.path.display(), err);
            } else if let Some(reason) = &file.skipped {
                println!("  {}: skipped ({})", file.path.display(), reason);
            } else if file.modified {
                println!("  {}: modified ({} replacements)", file.path.display(), file.replacements);
                if let Some(diff) = &file.diff {
                    println!("{}", diff);
                }
            } else {
                println!("  {}: no changes", file.path.display());
            }
        }
    }

    /// Determine the appropriate exit code for this report.
    pub fn exit_code(&self) -> i32 {
        if self.policy_violation.is_some() {
            2
        } else if self.has_errors {
            1
        } else if self.modified == 0 && self.total > 0 {
            // Check if all files were skipped or just no matches
            // Standard diff/grep: exit 1 if no changes/matches found.
            // If we have errors, we already returned 1.
            // If we have skipped files, strictly speaking they are not "errors" but "warnings" usually.
            // But if I asked to change files and nothing changed, exit 1 is appropriate.
            1 
        } else {
            0
        }
    }

    /// Print report as JSON.
    pub fn print_json(&self) {
        let json = serde_json::to_string_pretty(self).expect("Failed to serialize report");
        println!("{}", json);
    }

    /// Print report in Agent-friendly XML format.
    pub fn print_agent(&self) {
        for file in &self.files {
            println!("<file path=\"{}\">", file.path.display());
            if let Some(err) = &file.error {
                println!("ERROR: {}", err);
            } else if let Some(reason) = &file.skipped {
                println!("SKIPPED: {}", reason);
            } else if let Some(diff) = &file.diff {
                println!("{}", diff);
            } else if file.modified {
                 println!("(modified)");
            } else {
                 println!("(no changes)");
            }
            println!("</file>");
        }
    }
}