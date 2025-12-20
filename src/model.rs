use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LineRange {
    pub start: usize,
    pub end: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReplacementRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Transaction {
    #[default]
    All,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Symlinks {
    #[default]
    Follow,
    Skip,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BinaryFileMode {
    #[default]
    Skip,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PermissionsMode {
    #[default]
    Preserve,
    Fixed(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    #[default]
    Strict,
    Warn,
    None,
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
        /// Enable regex capture expansion (e.g. $1, $name).
        #[serde(default)]
        expand: bool,
        /// Replacement validation mode.
        #[serde(default)]
        validation_mode: ValidationMode,
    },
    /// Delete occurrences of a pattern.
    Delete {
        /// Pattern to find (literal string or regex).
        find: String,
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
    // Future operations: Insert, RegexReplace, etc.
}

/// A complete transformation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct Pipeline {
    /// Files to process.
    pub files: Vec<String>,
    /// Operations to apply to each file.
    pub operations: Vec<Operation>,

    // Safety and guarantees
    /// Dry-run mode: compute changes but don't write.
    #[serde(default)]
    pub dry_run: bool,
    /// Stronger than dry-run: guarantees zero writes even if output mode changes.
    #[serde(default)]
    pub no_write: bool,
    /// Fail if zero matches are found across all inputs.
    #[serde(default)]
    pub require_match: bool,
    /// Require exactly N total replacements across all inputs.
    #[serde(default)]
    pub expect: Option<usize>,
    /// Exit non-zero if any change would occur.
    #[serde(default)]
    pub fail_on_change: bool,

    // Transaction model
    #[serde(default)]
    pub transaction: Transaction,

    // Filesystem behavior
    #[serde(default)]
    pub symlinks: Symlinks,
    #[serde(default)]
    pub binary: BinaryFileMode,
    #[serde(default)]
    pub permissions: PermissionsMode,

    /// Validate manifest and semantic checks without running.
    #[serde(default)]
    pub validate_only: bool,

    /// Glob patterns to include.
    #[serde(default)]
    pub glob_include: Option<Vec<String>>,
    /// Glob patterns to exclude.
    #[serde(default)]
    pub glob_exclude: Option<Vec<String>>,
}

impl Pipeline {
    /// Create a simple replace pipeline.
    #[allow(dead_code)]
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
                expand: false,
                validation_mode: ValidationMode::default(),
            }],
            dry_run: false,
            no_write: false,
            require_match: false,
            expect: None,
            fail_on_change: false,
            transaction: Transaction::default(),
            symlinks: Symlinks::default(),
            binary: BinaryFileMode::default(),
            permissions: PermissionsMode::default(),
            validate_only: false,
            glob_include: None,
            glob_exclude: None,
        }
    }
}

impl From<crate::cli::Transaction> for Transaction {
    fn from(item: crate::cli::Transaction) -> Self {
        match item {
            crate::cli::Transaction::All => Transaction::All,
            crate::cli::Transaction::File => Transaction::File,
        }
    }
}

impl From<crate::cli::Symlinks> for Symlinks {
    fn from(item: crate::cli::Symlinks) -> Self {
        match item {
            crate::cli::Symlinks::Follow => Symlinks::Follow,
            crate::cli::Symlinks::Skip => Symlinks::Skip,
            crate::cli::Symlinks::Error => Symlinks::Error,
        }
    }
}

impl From<crate::cli::BinaryFileMode> for BinaryFileMode {
    fn from(item: crate::cli::BinaryFileMode) -> Self {
        match item {
            crate::cli::BinaryFileMode::Skip => BinaryFileMode::Skip,
            crate::cli::BinaryFileMode::Error => BinaryFileMode::Error,
        }
    }
}

impl From<crate::cli::ValidationMode> for ValidationMode {
    fn from(item: crate::cli::ValidationMode) -> Self {
        match item {
            crate::cli::ValidationMode::Strict => ValidationMode::Strict,
            crate::cli::ValidationMode::Warn => ValidationMode::Warn,
            crate::cli::ValidationMode::None => ValidationMode::None,
        }
    }
}
