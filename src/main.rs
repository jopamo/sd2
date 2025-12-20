use anyhow::{Context, Result, bail};
use clap::Parser;
use std::fs;
use std::io::IsTerminal;

use crate::cli::{Cli, Commands, DefaultArgs, OutputFormat, PermissionsMode as CliPermissionsMode};
use crate::input::{InputItem, InputMode};
use crate::model::{LineRange, Operation, PermissionsMode, Pipeline};

mod cli;
mod engine;
mod error;
mod events;
mod exit_codes;
mod input;
mod model;
mod policy;
mod replacer;
mod reporter;
mod rgjson;
mod transaction;
mod write;

fn parse_range(s: &str) -> Option<LineRange> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.is_empty() {
        return None;
    }

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

fn resolve_permissions(args: &DefaultArgs) -> Result<Option<PermissionsMode>> {
    if let Some(ref m_str) = args.mode {
        let m = u32::from_str_radix(m_str, 8).context("Invalid octal mode")?;
        Ok(Some(PermissionsMode::Fixed(m)))
    } else {
        match args.permissions {
            Some(CliPermissionsMode::Fixed) => {
                bail!("--mode <OCTAL> is required when --permissions fixed is used")
            }
            Some(CliPermissionsMode::Preserve) => Ok(None), // No override / default
            None => Ok(None),
        }
    }
}

fn main() {
    match try_main() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            if let Some(crate::error::Error::TransactionFailure(_)) =
                e.downcast_ref::<crate::error::Error>()
            {
                eprintln!("Error: {:#}", e);
                std::process::exit(exit_codes::TRANSACTION_FAILURE);
            }
            eprintln!("Error: {:#}", e);
            std::process::exit(exit_codes::ERROR);
        }
    }
}

fn try_main() -> Result<i32> {
    let cli = Cli::parse();

    let (manifest_path, find, replace, files, default_args) = match cli.command {
        Some(Commands::Schema) => {
            let schema = schemars::schema_for!(Pipeline);
            println!("{}", serde_json::to_string_pretty(&schema)?);
            return Ok(exit_codes::SUCCESS);
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
            // Default command behavior: stedi [OPTIONS] FIND REPLACE [FILES...]
            let default_args = cli.args;
            (
                default_args.manifest.clone(),
                default_args.find.clone(),
                default_args.replace.clone(),
                default_args.files.clone(),
                default_args,
            )
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
    let mut inputs: Vec<InputItem> = match mode {
        InputMode::Auto(ref paths) => {
            if !paths.is_empty() {
                paths.iter().map(|p| InputItem::Path(p.clone())).collect()
            } else if !std::io::stdin().is_terminal() {
                input::read_paths_from_stdin()?
                    .into_iter()
                    .map(InputItem::Path)
                    .collect()
            } else {
                Vec::new() // No inputs
            }
        }
        InputMode::StdinPathsNewline => input::read_paths_from_stdin()?
            .into_iter()
            .map(InputItem::Path)
            .collect(),
        InputMode::StdinPathsNul => input::read_paths_from_stdin_zero()?
            .into_iter()
            .map(InputItem::Path)
            .collect(),
        InputMode::StdinText => {
            vec![InputItem::StdinText(input::read_stdin_text()?)]
        }
        InputMode::RipgrepJson => input::read_rg_json()?,
    };

    // 2. Build Pipeline
    let pipeline = if let Some(path) = &manifest_path {
        let content =
            fs::read_to_string(path).context(format!("reading manifest from {:?}", path))?;
        let mut p: Pipeline = serde_json::from_str(&content).context("parsing manifest")?;

        // Apply CLI overrides if present
        if args.dry_run {
            p.dry_run = true;
        }
        if args.no_write {
            p.no_write = true;
        }
        if args.validate_only {
            p.validate_only = true;
        }
        if args.require_match {
            p.require_match = true;
        }
        if args.expect.is_some() {
            p.expect = args.expect;
        }
        if args.fail_on_change {
            p.fail_on_change = true;
        }
        if let Some(t) = &args.transaction {
            p.transaction = t.clone().into();
        }
        if let Some(s) = &args.symlinks {
            p.symlinks = s.clone().into();
        }
        if let Some(b) = &args.binary {
            p.binary = b.clone().into();
        }

        // Resolve permissions override
        if let Some(perms) = resolve_permissions(&args)? {
            p.permissions = perms;
        }

        if !args.glob_include.is_empty() {
            p.glob_include = Some(args.glob_include);
        }
        if !args.glob_exclude.is_empty() {
            p.glob_exclude = Some(args.glob_exclude);
        }

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

        let validation_mode = args.validation_mode.map(Into::into).unwrap_or_default();

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
            expand: args.expand,
            validation_mode,
        };

        // Resolve permissions
        let permissions = resolve_permissions(&args)?.unwrap_or(PermissionsMode::Preserve);

        Pipeline {
            files: vec![], // Populated by inputs
            operations: vec![op],
            dry_run: args.dry_run,
            no_write: args.no_write,
            require_match: args.require_match,
            expect: args.expect,
            fail_on_change: args.fail_on_change,
            transaction: args.transaction.clone().map(Into::into).unwrap_or_default(),
            symlinks: args.symlinks.clone().map(Into::into).unwrap_or_default(),
            binary: args.binary.clone().map(Into::into).unwrap_or_default(),
            permissions,
            validate_only: args.validate_only,
            glob_include: if args.glob_include.is_empty() {
                None
            } else {
                Some(args.glob_include)
            },
            glob_exclude: if args.glob_exclude.is_empty() {
                None
            } else {
                Some(args.glob_exclude)
            },
        }
    };

    // Populate inputs from pipeline files if empty (common in apply mode)
    if inputs.is_empty() && !pipeline.files.is_empty() {
        for f in &pipeline.files {
            inputs.push(InputItem::Path(std::path::PathBuf::from(f)));
        }
    }

    // 3. Execute
    let pipeline_for_report = pipeline.clone();
    let report = engine::execute(pipeline, inputs)?;

    // 4. Report
    let format = args.format.unwrap_or_else(|| {
        if args.json {
            OutputFormat::Json
        } else if std::io::stdout().is_terminal() {
            OutputFormat::Diff
        } else if let InputMode::StdinText = mode {
            OutputFormat::Diff
        } else {
            OutputFormat::Json
        }
    });

    let mode_str = if manifest_path.is_some() {
        "apply"
    } else {
        "cli"
    };
    let input_mode_str = match mode {
        InputMode::Auto(_) => "args",
        InputMode::StdinPathsNewline => "stdin-paths",
        InputMode::StdinPathsNul => "files0",
        InputMode::StdinText => "stdin-text",
        InputMode::RipgrepJson => "rg-json",
    };

    match format {
        OutputFormat::Json => report.print_json(
            &pipeline_for_report,
            env!("CARGO_PKG_VERSION"),
            mode_str,
            input_mode_str,
        ),
        OutputFormat::Agent => report.print_agent(),
        OutputFormat::Diff => {
            if args.quiet {
                report.print_errors_only()
            } else {
                report.print_human()
            }
        }
        OutputFormat::Summary => {
            if args.quiet {
                report.print_errors_only()
            } else {
                report.print_summary()
            }
        }
    }

    Ok(report.exit_code())
}
