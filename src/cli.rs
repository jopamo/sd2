use clap::Parser;
use std::path::PathBuf;

/// sd2: A structured text transformation tool.
/// Use with ripgrep for file selection, or provide a JSON manifest for complex operations.
#[derive(Parser, Debug)]
#[command(
    name = "sd2",
    author,
    version,
    about,
    max_term_width = 100,
    after_help = "\
EXIT STATUS:
  0  Success (and at least one file was changed, or stdin processed)
  1  No matches / no changes (when operating on files or stdin)
  2  Error (invalid arguments, IO errors, parse errors, write failures)"
)]
pub enum Cli {
    /// Apply transformations to files.
    #[command(visible_alias = "a")]
    Apply(ApplyArgs),
    /// Print JSON Schema for the manifest format.
    #[command(visible_alias = "s")]
    Schema,
}

/// Arguments for the apply command.
#[derive(Parser, Debug)]
pub struct ApplyArgs {
    /// JSON manifest file specifying transformations.
    #[arg(short, long, value_name = "FILE")]
    pub manifest: Option<PathBuf>,

    /// Pattern to find (literal string or regex).
    #[arg(value_name = "FIND", required_unless_present = "manifest")]
    pub find: Option<String>,

    /// Replacement text.
    #[arg(value_name = "REPLACE", required_unless_present = "manifest")]
    pub replace: Option<String>,

    /// Files to process (or read from stdin if empty).
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    // ========================================================================
    // Input Mode options
    // ========================================================================
    /// Force stdin to be interpreted as newline-delimited paths.
    #[arg(long = "stdin-paths", conflicts_with_all = ["files0", "stdin_text", "rg_json"])]
    pub stdin_paths: bool,

    /// Read NUL-delimited paths from stdin (for find -print0, fd -0).
    #[arg(long = "files0", conflicts_with_all = ["stdin_paths", "stdin_text", "rg_json"])]
    pub files0: bool,

    /// Treat stdin as content and write transformed content to stdout.
    #[arg(long = "stdin-text", conflicts_with_all = ["stdin_paths", "files0", "rg_json"])]
    pub stdin_text: bool,

    /// Consume rg --json output from stdin and apply edits to matched spans.
    #[arg(long = "rg-json", conflicts_with_all = ["stdin_paths", "files0", "stdin_text"])]
    pub rg_json: bool,

    // ========================================================================
    // Match options
    // ========================================================================
    /// Treat pattern as regex.
    #[arg(long = "regex", conflicts_with = "fixed_strings")]
    pub regex: bool,

    /// Treat pattern as literal string (not regex).
    #[arg(short = 'F', long = "fixed-strings", conflicts_with = "regex")]
    pub fixed_strings: bool,

    /// Case-insensitive matching.
    #[arg(short = 'i', long = "ignore-case")]
    pub ignore_case: bool,

    /// Smart-case: case-insensitive if pattern is all lowercase.
    #[arg(short = 'S', long = "smart-case")]
    pub smart_case: bool,

    /// Match only at word boundaries.
    #[arg(short = 'w', long = "word-regexp")]
    pub word_regexp: bool,

    /// Enable multi-line mode (^ and $ match line boundaries).
    #[arg(long = "multiline")]
    pub multiline: bool,

    /// Make '.' match newlines.
    #[arg(long = "dot-matches-newline")]
    pub dot_matches_newline: bool,

    /// Disable Unicode-aware matching.
    #[arg(long = "no-unicode")]
    pub no_unicode: bool,

    /// Maximum number of replacements per file.
    #[arg(short = 'n', long = "max-replacements", visible_alias = "limit")]
    pub max_replacements: Option<usize>,

    /// Only apply replacements in a line range (1-based, START[:END]).
    #[arg(long = "range", value_name = "START[:END]")]
    pub range: Option<String>,

    // ========================================================================
    // Output options
    // ========================================================================
    /// Dry-run mode: compute changes but don't write.
    #[arg(short = 'p', long = "preview")]
    pub preview: bool,

    /// Create backup before modifying.
    #[arg(long = "backup")]
    pub backup: bool,

    /// Backup file extension (default: ".bak").
    #[arg(long = "backup-ext", value_name = "EXT", default_value = ".bak")]
    pub backup_ext: String,

    /// Follow symbolic links.
    #[arg(long = "follow-symlinks")]
    pub follow_symlinks: bool,

    /// Continue processing files after errors.
    #[arg(long = "continue-on-error")]
    pub continue_on_error: bool,

    /// Output JSON instead of human-readable messages.
    #[arg(long = "json")]
    pub json: bool,

    /// Validate manifest and semantic checks without running.
    #[arg(long = "validate-only", conflicts_with = "preview")]
    pub validate_only: bool,
}