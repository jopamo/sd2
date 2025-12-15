use crate::error::{Error, Result};
use std::io::{self, BufRead, Read, BufReader};
use std::path::PathBuf;
use crate::rgjson::{self, RgMessage, RgKind, stream_rg_json_ndjson, RgSink, DeinterleavingSink, RgData};
use crate::model::{LineRange, ReplacementRange};

#[derive(Debug, PartialEq, Eq)]
pub enum InputMode {
    /// Read paths from command line arguments.
    /// If no args, and stdin is a pipe, read paths from stdin (newline delimited).
    Auto(Vec<PathBuf>),
    /// Read paths from stdin (newline delimited).
    StdinPathsNewline,
    /// Read paths from stdin (NUL delimited).
    StdinPathsNul,
    /// Read content from stdin.
    StdinText,
    /// Read ripgrep JSON from stdin.
    RipgrepJson,
}

#[derive(Debug)]
pub enum InputItem {
    Path(PathBuf),
    StdinText(String),
    RipgrepMatch {
        path: PathBuf,
        matches: Vec<ReplacementRange>,
    },
}

pub fn resolve_input_mode(
    stdin_paths: bool,
    files0: bool,
    stdin_text: bool,
    rg_json: bool,
    files_arg: bool,
    files: &Vec<PathBuf>,
) -> InputMode {
    if stdin_text {
        InputMode::StdinText
    } else if rg_json {
        InputMode::RipgrepJson
    } else if files0 {
        InputMode::StdinPathsNul
    } else if stdin_paths {
        InputMode::StdinPathsNewline
    } else if files_arg {
        InputMode::Auto(files.clone())
    } else {
        // Default behavior: Auto mode
        InputMode::Auto(files.clone())
    }
}

/// Read newline-delimited paths from stdin.
pub fn read_paths_from_stdin() -> Result<Vec<PathBuf>> {
    let stdin = io::stdin();
    let mut paths = Vec::new();
    for line in stdin.lock().lines() {
        let line = line.map_err(Error::Io)?;
        if !line.trim().is_empty() {
            paths.push(PathBuf::from(line.trim()));
        }
    }
    Ok(paths)
}

/// Read NUL-delimited paths from stdin.
pub fn read_paths_from_stdin_zero() -> Result<Vec<PathBuf>> {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut paths = Vec::new();
    let mut buf = Vec::new();
    
    // read_until includes the delimiter
    while handle.read_until(0, &mut buf).map_err(Error::Io)? > 0 {
        // Remove the trailing NUL
        if let Some(&0) = buf.last() {
            buf.pop();
        }
        if !buf.is_empty() {
             let s = String::from_utf8(buf.clone())
                .map_err(|e| Error::Validation(format!("Invalid UTF-8 in path: {}", e)))?;
             paths.push(PathBuf::from(s));
        }
        buf.clear();
    }
    Ok(paths)
}

/// Read all text from stdin.
pub fn read_stdin_text() -> Result<String> {
    let mut buffer = String::new();
    // Check if stdin is tty? No, if mode is StdinText we assume they want to read from it.
    // But if it is a TTY we might hang.
    // However, logic usually checks atty before calling this if in Auto mode.
    // In StdinText mode, we force read.
    io::stdin().read_to_string(&mut buffer).map_err(Error::Io)?;
    Ok(buffer)
}

/// Read ripgrep JSON output and extract paths and matches.
/// Uses DeinterleavingSink to group by file.
pub fn read_rg_json() -> Result<Vec<InputItem>> {
    let stdin = io::stdin();
    let reader = BufReader::new(stdin.lock());
    let mut sink = DeinterleavingSink::new();
    
    stream_rg_json_ndjson(reader, &mut sink).map_err(|e| Error::Validation(format!("Failed to parse rg json: {}", e)))?;
    
    let mut items = Vec::new();

    for (path_os, events) in sink.events {
        let path = PathBuf::from(path_os);
        let mut matches = Vec::new();

        for event in events {
             // For each event (RgData), we extract submatches
             // If absolute_offset is present, we can calculate absolute ranges
             if let Some(abs_start) = event.absolute_offset {
                 for sub in event.submatches {
                     // sub.start/end are relative to the match text?
                     // Usually rg submatches are relative to the line content start?
                     // Let's assume absolute_offset is the line start.
                     // And sub.start is offset from line start.
                     let start = (abs_start as usize) + (sub.start as usize);
                     let end = (abs_start as usize) + (sub.end as usize);
                     matches.push(ReplacementRange { start, end });
                 }
             } else {
                 // Fallback or warning?
                 // If no absolute offset, we can't do safe targeted replacement reliably without re-reading file lines.
                 // For now, skip if we can't determine range.
             }
        }
        
        // Merge overlapping or adjacent ranges?
        // Not strictly necessary if the engine handles overlapping replacements, but good practice.
        // For now, just pass them.
        
        items.push(InputItem::RipgrepMatch {
            path,
            matches,
        });
    }

    Ok(items)
}
