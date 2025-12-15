use crate::cli::ApplyArgs;
use crate::error::{Error, Result};
use std::io::{self, BufRead, Read};
use std::path::PathBuf;
use serde::Deserialize;

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
    // RgSpan { ... } // Future
}

pub fn resolve_input_mode(args: &ApplyArgs) -> InputMode {
    if args.stdin_text {
        InputMode::StdinText
    } else if args.rg_json {
        InputMode::RipgrepJson
    } else if args.files0 {
        InputMode::StdinPathsNul
    } else if args.stdin_paths {
        InputMode::StdinPathsNewline
    } else {
        InputMode::Auto(args.files.clone())
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum RgMessage {
    #[serde(rename = "match")]
    Match {
        path: RgPath,
        lines: RgLines,
        line_number: Option<u64>,
        absolute_offset: u64,
        submatches: Vec<RgSubmatch>,
    },
    #[serde(rename = "begin")]
    Begin {
        path: RgPath,
    },
    #[serde(rename = "end")]
    End {
        path: RgPath,
        binary_offset: Option<u64>,
        stats: RgStats,
    },
    #[serde(rename = "summary")]
    Summary {
        elapsed_total: RgDuration,
        stats: RgStats,
    },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RgPath {
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RgLines {
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RgSubmatch {
    pub match_text: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RgStats {
    pub elapsed: RgDuration,
    pub searches: u64,
    pub searches_with_match: u64,
    pub matches: u64,
    pub matched_lines: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RgDuration {
    pub secs: u64,
    pub nanos: u64,
    pub human: String,
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

/// Read ripgrep JSON output and extract paths.
/// TODO: In the future, this should also extract match spans for targeted replacement.
pub fn read_rg_json() -> Result<Vec<PathBuf>> {
    let stdin = io::stdin();
    let mut paths = Vec::new();
    
    for line in stdin.lock().lines() {
        let line = line.map_err(Error::Io)?;
        if line.trim().is_empty() { continue; }
        
        // We accept that some lines might not be valid JSON or might not be the messages we care about
        // But for --rg-json, we expect a stream of these.
        if let Ok(msg) = serde_json::from_str::<RgMessage>(&line) {
             match msg {
                 RgMessage::Begin { path } => {
                     paths.push(PathBuf::from(path.text));
                 }
                 _ => {}
             }
        }
    }
    // Deduplicate? Rg usually groups by file, but we might get multiple blocks?
    // A simple vector is fine for now, dedup can happen later if needed.
    paths.sort();
    paths.dedup();
    Ok(paths)
}
