use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// sd2: A structured text transformation tool.
/// Use with ripgrep for file selection, or provide a JSON manifest for complex operations.
#[derive(Debug, Clone, clap::ValueEnum, PartialEq)]
#[clap(rename_all = "kebab-case")]
pub enum Transaction {
    All,
    File,
}

#[derive(Debug, Clone, clap::ValueEnum, PartialEq)]
#[clap(rename_all = "kebab-case")]
pub enum Symlinks {
    Follow,
    Skip,
    Error,
}

#[derive(Debug, Clone, clap::ValueEnum, PartialEq)]
#[clap(rename_all = "kebab-case")]
pub enum BinaryFileMode {
    Skip,
    Error,
}

#[derive(Debug, Clone, clap::ValueEnum, PartialEq)]
#[clap(rename_all = "kebab-case")]
pub enum PermissionsMode {
    Preserve,
    Fixed,
}

#[derive(Debug, Clone, clap::ValueEnum, PartialEq, Copy)]
#[clap(rename_all = "kebab-case")]
pub enum ValidationMode {
    Strict,
    Warn,
    None,
}

#[derive(Debug, Clone, clap::ValueEnum, PartialEq)]
pub enum OutputFormat {
    Diff,
    Summary,
    Json,
    Agent, // This is specific to the agent, not directly in helptext.txt's explicit formats.
}
#[derive(Parser, Debug)]
#[command(
    name = "sd2",
    author,
    version,
    about,
    max_term_width = 100,
    after_help = "\
EXIT STATUS:
  0  Success (and no policy violations)
  1  Operational failure (I/O, parse errors, invalid args)
  2  Policy failure (--require-match, --expect, --fail-on-change)
  3  Partial/aborted transaction (should only happen with --transaction file)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[clap(flatten)]
    pub args: DefaultArgs,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Print the JSON Schema describing manifests, operations, and output events.
    #[command(visible_alias = "s")]
    Schema,
    /// Apply a manifest (multi-file, multi-op), with full validation and atomic commit.
    #[command(visible_alias = "a")]
    Apply(ApplyArgs),
}

/// Default command: sd2 FIND REPLACE [FILES...]
#[derive(Args, Debug)]
pub struct DefaultArgs {
    /// JSON manifest file specifying transformations. Used with `apply` command.
    /// This is here only for cases where `apply` is used as the default command with `--manifest`.
    #[arg(short, long, value_name = "FILE", global = true, help_heading = "Input Options")]
    pub manifest: Option<PathBuf>,

    /// Pattern to find (literal string or regex).
    #[arg(value_name = "FIND")]
    pub find: Option<String>,

    /// Replacement text.
    #[arg(value_name = "REPLACE")]
    pub replace: Option<String>,

    /// Files to process (or read from stdin if empty).
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    // ========================================================================
    // Input Mode options
    // ========================================================================
    /// Force stdin to be interpreted as newline-delimited paths.
    #[arg(long = "stdin-paths", conflicts_with_all = ["files0", "stdin_text", "rg_json", "files_arg"], help_heading = "Input Options")]
    pub stdin_paths: bool,

    /// Read NUL-delimited paths from stdin (for find -print0, fd -0).
    #[arg(long = "files0", conflicts_with_all = ["stdin_paths", "stdin_text", "rg_json", "files_arg"], help_heading = "Input Options")]
    pub files0: bool,

    /// Treat stdin as content and write transformed content to stdout.
    #[arg(long = "stdin-text", conflicts_with_all = ["stdin_paths", "files0", "rg_json", "files_arg"], help_heading = "Input Options")]
    pub stdin_text: bool,

    /// Consume rg --json output from stdin and apply edits to matched spans.
    #[arg(long = "rg-json", conflicts_with_all = ["stdin_paths", "files0", "stdin_text", "files_arg"], help_heading = "Input Options")]
    pub rg_json: bool,

    /// Force positional arguments to be treated as files even if stdin is present.
    #[arg(long = "files", conflicts_with_all = ["stdin_paths", "files0", "stdin_text", "rg_json"], visible_alias = "files-arg", help_heading = "Input Options")]
    pub files_arg: bool,

    // ========================================================================
    // Match options
    // ========================================================================
    /// Treat FIND as a regex pattern.
    #[arg(long = "regex", conflicts_with = "fixed_strings", help_heading = "Match Options")]
    pub regex: bool,

    /// Treat FIND as a literal string (not regex).
    #[arg(short = 'F', long = "fixed-strings", conflicts_with = "regex", help_heading = "Match Options")]
    pub fixed_strings: bool,

    /// Case-insensitive matching.
    #[arg(short = 'i', long = "ignore-case", help_heading = "Match Options")]
    pub ignore_case: bool,

    /// Smart-case: case-insensitive unless FIND contains uppercase.
    #[arg(short = 'S', long = "smart-case", help_heading = "Match Options")]
    pub smart_case: bool,

    /// Match only at word boundaries.
    #[arg(short = 'w', long = "word-regexp", help_heading = "Match Options")]
    pub word_regexp: bool,

    /// Enable multi-line mode (^ and $ match line boundaries).
    #[arg(long = "multiline", help_heading = "Match Options")]
    pub multiline: bool,

    /// Make '.' match newlines.
    #[arg(long = "dot-matches-newline", help_heading = "Match Options")]
    pub dot_matches_newline: bool,

    /// Disable Unicode-aware matching.
    #[arg(long = "no-unicode", help_heading = "Match Options")]
    pub no_unicode: bool,

    /// Maximum replacements per file.
    #[arg(long = "limit", value_name = "N", visible_alias = "max-replacements", help_heading = "Scope Options")]
    pub limit: Option<usize>,

    /// Only apply replacements in a line range (1-based, START[:END]).
    #[arg(long = "range", value_name = "START[:END]", help_heading = "Scope Options")]
    pub range: Option<String>,

    /// Enable regex capture expansion (e.g. $1, $name).
    #[arg(long = "expand", help_heading = "Match Options")]
    pub expand: bool,

    /// Replacement validation mode.
    #[arg(long = "validation-mode", value_enum, global = true, help_heading = "Match Options")]
    pub validation_mode: Option<ValidationMode>,

    /// Apply edits only to files whose *path* matches the glob.
    #[arg(long = "glob-include", value_name = "GLOB", help_heading = "Scope Options")]
    pub glob_include: Vec<String>,

    /// Exclude matching paths.
    #[arg(long = "glob-exclude", value_name = "GLOB", help_heading = "Scope Options")]
    pub glob_exclude: Vec<String>,

    // ========================================================================
    // Safety and guarantees
    // ========================================================================
    /// Print a unified diff, perform no writes.
    #[arg(long = "dry-run", short = 'p', help_heading = "Safety Options")]
    pub dry_run: bool,

    /// Stronger than --dry-run: guarantees zero filesystem writes.
    #[arg(long = "no-write", help_heading = "Safety Options")]
    pub no_write: bool,

    /// Fail if zero matches are found across all inputs.
    #[arg(long = "require-match", help_heading = "Safety Options")]
    pub require_match: bool,

    /// Require exactly N total replacements across all inputs.
    #[arg(long = "expect", value_name = "N", help_heading = "Safety Options")]
    pub expect: Option<usize>,

    /// Exit non-zero if any change would occur (CI assertions).
    #[arg(long = "fail-on-change", help_heading = "Safety Options")]
    pub fail_on_change: bool,

    // ========================================================================
    // Transaction model
    // ========================================================================
    /// Transaction model: 'all' (default) or 'file'.
    #[arg(long = "transaction", value_enum, global = true, help_heading = "Configuration")]
    pub transaction: Option<Transaction>,

    // ========================================================================
    // Filesystem behavior
    // ========================================================================
    /// Symlink handling: 'follow' (default), 'skip', or 'error'.
    #[arg(long = "symlinks", value_enum, global = true, help_heading = "Configuration")]
    pub symlinks: Option<Symlinks>,

    /// Binary file handling: 'skip' (default) or 'error'.
    #[arg(long = "binary", value_enum, global = true, help_heading = "Configuration")]
    pub binary: Option<BinaryFileMode>,

    /// Permissions handling: 'preserve' (default) or 'fixed'.
    #[arg(long = "permissions", value_enum, global = true, help_heading = "Configuration")]
    pub permissions: Option<PermissionsMode>,

    /// Fixed permissions mode (e.g. 755), used if --permissions=fixed.
    #[arg(long = "mode", value_name = "MODE", global = true, help_heading = "Configuration")]
    pub mode: Option<String>,

    // ========================================================================
    // Output control
    // ========================================================================
    /// Force JSON event output even on a TTY.
    #[arg(long = "json", help_heading = "Output Options")]
    pub json: bool,

    /// No diff, no summary. Errors still emitted.
    #[arg(long = "quiet", help_heading = "Output Options")]
    pub quiet: bool,

    /// Explicit output formatting.
    #[arg(long = "format", value_enum, global = true, help_heading = "Output Options")]
    pub format: Option<OutputFormat>,

    /// Validate manifest and semantic checks without running.
    #[arg(long = "validate-only", conflicts_with = "dry_run", help_heading = "Safety Options")]
    pub validate_only: bool,
}

/// Arguments for the 'apply' subcommand.
#[derive(Args, Debug)]
pub struct ApplyArgs {
    /// JSON manifest file specifying transformations.
    #[arg(short, long, value_name = "FILE")]
    pub manifest: PathBuf,

    /// Validate manifest and semantic checks without running.
    #[arg(long = "validate-only")]
    pub validate_only: bool,

    /// Print a unified diff, perform no writes.
    #[arg(long = "dry-run", short = 'p')]
    pub dry_run: bool,

    /// Force JSON event output even on a TTY.
    #[arg(long = "json")]
    pub json: bool,
}