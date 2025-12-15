use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LineRange {
    pub start: usize,
    pub end: Option<usize>,
}

/// A single text transformation operation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Operation {
    /// Replace occurrences of a pattern with replacement text.
    Replace {
        /// Pattern to find (literal string or regex).
        find: String,
        /// Replacement text.
        with: String,
        /// Whether to treat pattern as literal string (not regex).
        #[serde(default)]
        literal: bool,
        /// Case-insensitive matching.
        #[serde(default)]
        ignore_case: bool,
        /// Smart-case: case-insensitive if pattern is all lowercase.
        #[serde(default)]
        smart_case: bool,
        /// Match only at word boundaries.
        #[serde(default)]
        word: bool,
        /// Enable multi-line mode (^ and $ match line boundaries).
        #[serde(default)]
        multiline: bool,
        /// Make '.' match newlines.
        #[serde(default)]
        dot_matches_newline: bool,
        /// Disable Unicode-aware matching.
        #[serde(default)]
        no_unicode: bool,
        /// Maximum number of replacements per file (0 = unlimited).
        #[serde(default)]
        limit: usize,
        /// Only apply replacements in a line range (1-based).
        #[serde(default)]
        range: Option<LineRange>,
    },
    // Future operations: Delete, Insert, RegexReplace, etc.
}

/// A complete transformation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Pipeline {
    /// Files to process.
    pub files: Vec<String>,
    /// Operations to apply to each file.
    pub operations: Vec<Operation>,
    /// Dry-run mode: compute changes but don't write.
    #[serde(default)]
    pub dry_run: bool,
    /// Create backup before modifying.
    #[serde(default)]
    pub backup: bool,
    /// Backup file extension (default: ".bak").
    #[serde(default)]
    pub backup_ext: Option<String>,
    /// Follow symbolic links.
    #[serde(default)]
    pub follow_symlinks: bool,
    /// Continue on errors.
    #[serde(default)]
    pub continue_on_error: bool,
    /// Validate manifest and semantic checks without running.
    #[serde(default)]
    pub validate_only: bool,
}

impl Pipeline {
    /// Create a simple replace pipeline.
    pub fn replace(files: Vec<String>, find: String, with_: String) -> Self {
        Self {
            files,
            operations: vec![Operation::Replace {
                find,
                with: with_,
                literal: false,
                ignore_case: false,
                smart_case: false,
                word: false,
                multiline: false,
                dot_matches_newline: false,
                no_unicode: false,
                limit: 0,
                range: None,
            }],
            dry_run: false,
            backup: false,
            backup_ext: None,
            follow_symlinks: false,
            continue_on_error: false,
            validate_only: false,
        }
    }
}