use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::io::IsTerminal;

use crate::cli::{Cli, Commands, OutputFormat, Transaction as CliTransaction, Symlinks as CliSymlinks, BinaryFileMode as CliBinaryFileMode, PermissionsMode as CliPermissionsMode};
use crate::input::{InputItem, InputMode};
use crate::model::{Operation, Pipeline, LineRange};

mod cli;
mod engine;
mod error;
mod input;
mod model;
mod replacer;
mod reporter;
mod rgjson;
mod write;

fn parse_range(s: &str) -> Option<LineRange> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.is_empty() { return None; }
    
    let start = parts[0].parse().ok()?;
    let end = if parts.len() > 1 {
        if parts[1].is_empty() {
            None
        } else {
            Some(parts[1].parse().ok()?)
        }
    } else {
        // Single number (e.g. "40") means that specific line only (40..40)
        Some(start)
    };
    
    Some(LineRange { start, end })
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (manifest_path, find, replace, files, default_args) = match cli.command {
        Some(Commands::Schema) => {
            let schema = schemars::schema_for!(Pipeline);
            println!("{}", serde_json::to_string_pretty(&schema)?);
            return Ok(());
        }
        Some(Commands::Apply(args)) => {
            // Manifest is required for apply subcommand
            let manifest_path = Some(args.manifest);
            // Overrides from apply subcommand args
            let default_args = cli::DefaultArgs {
                dry_run: args.dry_run,
                validate_only: args.validate_only,
                json: args.json,
                // Inherit other default_args, ensure no conflicts
                ..cli.args
            };
            (manifest_path, None, None, vec![], default_args)
        }
        None => {
            // Default command behavior: sd2 [OPTIONS] FIND REPLACE [FILES...]
            let default_args = cli.args;
            (default_args.manifest.clone(), default_args.find.clone(), default_args.replace.clone(), default_args.files.clone(), default_args)
        }
    };
    
    // Determine the actual args to use, preferring manifest-specific overrides
    let args = default_args;

    // Resolve input mode
    let mode = input::resolve_input_mode(
        args.stdin_paths,
        args.files0,
        args.stdin_text,
        args.rg_json,
        args.files_arg,
        &files,
    );

    // 1. Collect inputs
    let inputs: Vec<InputItem> = match mode {
        InputMode::Auto(ref paths) => {
            if !paths.is_empty() {
                 paths.iter().map(|p| InputItem::Path(p.clone())).collect()
            } else if !std::io::stdin().is_terminal() {
                input::read_paths_from_stdin()?.into_iter().map(InputItem::Path).collect()
            } else {
                Vec::new() // No inputs
            }
        }
        InputMode::StdinPathsNewline => {
             input::read_paths_from_stdin()?.into_iter().map(InputItem::Path).collect()
        }
        InputMode::StdinPathsNul => {
             input::read_paths_from_stdin_zero()?.into_iter().map(InputItem::Path).collect()
        }
        InputMode::StdinText => {
             vec![InputItem::StdinText(input::read_stdin_text()?)]
        }
                        InputMode::RipgrepJson => {
                             input::read_rg_json()?
                        }    };

    // 2. Build Pipeline
    let pipeline = if let Some(path) = manifest_path {
        let content = fs::read_to_string(&path).context(format!("reading manifest from {:?}", path))?;
        let mut p: Pipeline = serde_json::from_str(&content).context("parsing manifest")?;

        // Apply CLI overrides if present
        if args.dry_run { p.dry_run = true; }
        if args.no_write { p.no_write = true; }
        if args.validate_only { p.validate_only = true; }
        if args.require_match { p.require_match = true; }
        if args.expect.is_some() { p.expect = args.expect; }
        if args.fail_on_change { p.fail_on_change = true; }
        if args.transaction != CliTransaction::All { p.transaction = args.transaction.into(); } // Convert cli enum to model enum
        if args.symlinks != CliSymlinks::Follow { p.symlinks = args.symlinks.into(); } // Convert cli enum to model enum
        if args.binary != CliBinaryFileMode::Skip { p.binary = args.binary.into(); } // Convert cli enum to model enum
        if args.permissions != CliPermissionsMode::Preserve { p.permissions = args.permissions.into(); } // Convert cli enum to model enum
        if !args.glob_include.is_empty() { p.glob_include = Some(args.glob_include); }
        if !args.glob_exclude.is_empty() { p.glob_exclude = Some(args.glob_exclude); }
        
        p
    } else {
        // Construct from CLI args (for default command)
        let find = find.context("FIND pattern is required unless --manifest is used")?;
        let replace = replace.context("REPLACE pattern is required unless --manifest is used")?;
        
        let range = if let Some(r) = &args.range {
            parse_range(r)
        } else {
            None
        };

        let op = Operation::Replace {
            find,
            with: replace,
            literal: !args.regex,
            ignore_case: args.ignore_case,
            smart_case: args.smart_case,
            word: args.word_regexp,
            multiline: args.multiline,
            dot_matches_newline: args.dot_matches_newline,
            no_unicode: args.no_unicode,
            limit: args.limit.unwrap_or(0),
            range,
        };

        Pipeline {
            files: vec![], // Populated by inputs
            operations: vec![op],
            dry_run: args.dry_run,
            no_write: args.no_write,
            require_match: args.require_match,
            expect: args.expect,
            fail_on_change: args.fail_on_change,
            transaction: args.transaction.into(), // Convert cli enum to model enum
            symlinks: args.symlinks.into(), // Convert cli enum to model enum
            binary: args.binary.into(), // Convert cli enum to model enum
            permissions: args.permissions.into(), // Convert cli enum to model enum
            validate_only: args.validate_only,
            glob_include: if args.glob_include.is_empty() { None } else { Some(args.glob_include) },
            glob_exclude: if args.glob_exclude.is_empty() { None } else { Some(args.glob_exclude) },
        }
    };

    // 3. Execute
    let report = engine::execute(pipeline, inputs)?;

    // 4. Report
    let format = args.format.unwrap_or_else(|| {
        if args.json {
            OutputFormat::Json
        } else if args.quiet {
            // If quiet, don't output diff/summary, but still handle errors
            OutputFormat::Summary // or a new Silent format
        } else {
            OutputFormat::Diff
        }
    });
    
    match format {
        OutputFormat::Json => report.print_json(),
        OutputFormat::Agent => report.print_agent(), // Assuming print_agent handles quiet
        OutputFormat::Diff => if !args.quiet { report.print_human() },
        OutputFormat::Summary => if !args.quiet { report.print_human() },
    }

    std::process::exit(report.exit_code());
}