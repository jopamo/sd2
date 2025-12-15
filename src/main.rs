mod cli;
mod error;
mod model;
mod replacer;
mod write;
mod engine;
mod reporter;
mod input;

use clap::Parser;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();
    match cli {
        cli::Cli::Schema => print_schema(),
        cli::Cli::Apply(args) => apply(args),
    }
}

/// Print JSON Schema for the manifest format.
fn print_schema() -> Result<(), Box<dyn std::error::Error>> {
    let schema = schemars::schema_for!(model::Pipeline);
    let json = serde_json::to_string_pretty(&schema)?;
    println!("{}", json);
    Ok(())
}

/// Execute the apply command.
fn apply(args: cli::ApplyArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Capture JSON output flag before moving args
    let json_output = args.json;
    
    // Resolve input mode
    let input_mode = input::resolve_input_mode(&args);

    // Build pipeline from manifest or CLI args
    let pipeline = if let Some(manifest_path) = args.manifest {
        // Load JSON manifest
        let content = fs::read_to_string(manifest_path)?;
        let pipeline: model::Pipeline = serde_json::from_str(&content)?;
        pipeline
    } else {
        // Build from CLI arguments
        let find = args.find.expect("FIND required without manifest");
        let replace = args.replace.expect("REPLACE required without manifest");
        // Files are handled by input_mode, so we start with empty here for CLI args case
        // But pipeline.files is expected to be populated for engine?
        // Actually, we are separating inputs from pipeline configuration.
        // pipeline.files will be ignored/merged into inputs.
        let files: Vec<String> = Vec::new();
        let mut pipeline = model::Pipeline::replace(files, find, replace);

        // Apply CLI flags to the operation
        #[allow(irrefutable_let_patterns)]
        if let model::Operation::Replace { ref mut literal, ref mut ignore_case, ref mut smart_case,
            ref mut word, ref mut multiline, ref mut dot_matches_newline,
            ref mut no_unicode, ref mut limit, .. } = pipeline.operations[0] {
            *literal = !args.regex;
            *ignore_case = args.ignore_case;
            *smart_case = args.smart_case;
            *word = args.word_regexp;
            *multiline = args.multiline;
            *dot_matches_newline = args.dot_matches_newline;
            *no_unicode = args.no_unicode;
            *limit = args.max_replacements;
        }

        pipeline.dry_run = args.preview;
        pipeline.backup = args.backup;
        pipeline.backup_ext = Some(args.backup_ext);
        pipeline.follow_symlinks = args.follow_symlinks;
        pipeline.continue_on_error = args.continue_on_error;
        pipeline.validate_only = args.validate_only;
        pipeline
    };
    
    // Collect inputs
    let mut inputs: Vec<input::InputItem> = Vec::new();
    
    // 1. Add files from pipeline (manifest)
    for f in &pipeline.files {
        inputs.push(input::InputItem::Path(PathBuf::from(f)));
    }
    
    // 2. Add inputs from InputMode
    match input_mode {
        input::InputMode::Auto(files) => {
             if files.is_empty() {
                 // Check if stdin is piped. If so, read paths from stdin.
                 if !atty::is(atty::Stream::Stdin) {
                     for path in input::read_paths_from_stdin()? {
                         inputs.push(input::InputItem::Path(path));
                     }
                 }
             } else {
                 for path in files {
                     inputs.push(input::InputItem::Path(path));
                 }
             }
        }
        input::InputMode::StdinPathsNewline => {
             for path in input::read_paths_from_stdin()? {
                 inputs.push(input::InputItem::Path(path));
             }
        }
        input::InputMode::StdinPathsNul => {
             for path in input::read_paths_from_stdin_zero()? {
                 inputs.push(input::InputItem::Path(path));
             }
        }
        input::InputMode::StdinText => {
             let text = input::read_stdin_text()?;
             inputs.push(input::InputItem::StdinText(text));
        }
        input::InputMode::RipgrepJson => {
             for path in input::read_rg_json()? {
                 inputs.push(input::InputItem::Path(path));
             }
        }
    }

    // Execute pipeline
    let report = engine::execute(pipeline, inputs)?;

    // Output report
    if json_output {
        report.print_json();
    } else {
        report.print_human();
    }

    std::process::exit(report.exit_code());
}