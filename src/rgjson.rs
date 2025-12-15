use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Deserialize;
use std::borrow::Cow;
use std::ffi::OsString;
use std::io::{self, BufRead, Write};

// LINUX/UNIX SPECIFIC: Fast path for arbitrary bytes in paths
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

#[derive(Debug, Deserialize)]
pub struct RgMessage {
    #[serde(rename = "type")]
    pub kind: RgKind,
    pub data: Option<RgData>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RgKind {
    Begin,
    Match,
    Context,
    End,
    Summary,
}

#[derive(Debug, Deserialize)]
pub struct RgData {
    pub path: Option<RgTextOrBytes>,
    #[allow(dead_code)]
    pub lines: Option<RgTextOrBytes>,
    #[serde(default)]
    #[allow(dead_code)]
    pub line_number: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    pub absolute_offset: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    pub submatches: Vec<RgSubmatch>,
}

#[derive(Debug, Deserialize)]
pub struct RgSubmatch {
    #[serde(default)]
    #[allow(dead_code)]
    pub m: Option<RgTextOrBytes>,
    #[serde(default)]
    #[allow(dead_code)]
    pub start: u64,
    #[serde(default)]
    #[allow(dead_code)]
    pub end: u64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum RgTextOrBytes {
    Text { text: String },
    Bytes { bytes: String },
}

impl RgTextOrBytes {
    /// Returns the raw bytes, decoding base64 on the fly if necessary.
    /// Returns Cow::Borrowed if it's plain text, or Cow::Owned if we had to decode.
    pub fn as_bytes(&self) -> Result<Cow<'_, [u8]>> {
        match self {
            Self::Text { text } => Ok(Cow::Borrowed(text.as_bytes())),
            Self::Bytes { bytes } => {
                let raw = STANDARD
                    .decode(bytes.as_bytes())
                    .map_err(|e| anyhow!("base64 decode failed: {e}"))?;
                Ok(Cow::Owned(raw))
            }
        }
    }

    /// Lossy conversion to String.
    /// Great for agent contexts where we prefer <REPLACMENT_CHAR> over crashing.
    #[allow(dead_code)]
    pub fn as_string_lossy(&self) -> Result<Cow<'_, str>> {
        match self {
            Self::Text { text } => Ok(Cow::Borrowed(text)),
            Self::Bytes { .. } => {
                let raw = self.as_bytes()?;
                match raw {
                    Cow::Borrowed(b) => Ok(String::from_utf8_lossy(b)),
                    Cow::Owned(v) => Ok(Cow::Owned(String::from_utf8_lossy(&v).into_owned())),
                }
            }
        }
    }

    /// Robust path conversion.
    /// On Linux: Preserves arbitrary bytes (OsStringExt).
    /// On Others: Falls back to lossy UTF-8 (safer than crashing).
    pub fn to_os_string(&self) -> Result<OsString> {
        let raw_cow = self.as_bytes()?;
        
        #[cfg(unix)]
        {
            Ok(OsString::from_vec(raw_cow.into_owned()))
        }

        #[cfg(not(unix))] 
        {
            // Fallback for Windows/Wasm where paths *must* be valid WTF-8/UTF-8
            use std::ffi::OsStr;
            let s = match raw_cow {
                Cow::Borrowed(b) => String::from_utf8_lossy(b),
                Cow::Owned(v) => String::from_utf8_lossy(&v).into_owned(),
            };
            Ok(OsString::from(s.into_owned()))
        }
    }
}

pub trait RgSink {
    fn handle(&mut self, msg: RgMessage) -> Result<()>;
}

pub fn stream_rg_json_ndjson<R: BufRead, S: RgSink>(mut reader: R, sink: &mut S) -> Result<()> {
    // Re-use buffer to reduce allocation pressure
    let mut buf = Vec::with_capacity(8 * 1024);

    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf).context("read stdin")?;
        if n == 0 {
            break;
        }

        // Strip trailing newlines (works for \n and \r\n)
        while let Some(&last) = buf.last() {
            if last == b'\n' || last == b'\r' {
                buf.pop();
            } else {
                break;
            }
        }

        if buf.is_empty() {
            continue;
        }

        // We accept that some lines might not be valid JSON or might not be the messages we care about
        // But for --rg-json, we expect a stream of these.
        if let Ok(msg) = serde_json::from_slice::<RgMessage>(&buf) {
             sink.handle(msg)?;
        }
    }

    Ok(())
}

/// A sink that groups matches by file.
/// This prevents "interleaved" confusion for Agents and allows
/// constructing a cleaner context window.
#[allow(dead_code)]
pub struct BufferedAgentSink {
    stdout: io::StdoutLock<'static>,
    current_file_path: Option<String>,
    match_buffer: Vec<String>,
}

impl BufferedAgentSink {
    #[allow(dead_code)]
    pub fn new() -> Self {
        // Leaking stdin/stdout is common/acceptable in CLI tools for 'static locks
        let stdout = Box::leak(Box::new(io::stdout())).lock();
        Self {
            stdout,
            current_file_path: None,
            match_buffer: Vec::new(),
        }
    }

    #[allow(dead_code)]
    fn flush_current_file(&mut self) -> Result<()> {
        if let Some(path) = &self.current_file_path {
            if !self.match_buffer.is_empty() {
                // AGENT-FRIENDLY FORMAT:
                // Using explicit headers or XML tags makes it easier for 
                // the LLM to understand where file content starts/stops.
                writeln!(self.stdout, "<file path=\"{}\">", path)?;
                for line in &self.match_buffer {
                    writeln!(self.stdout, "{}", line)?;
                }
                writeln!(self.stdout, "</file>")?;
            }
        }
        self.match_buffer.clear();
        self.current_file_path = None;
        Ok(())
    }
}

impl RgSink for BufferedAgentSink {
    fn handle(&mut self, msg: RgMessage) -> Result<()> {
        match msg.kind {
            RgKind::Begin => {
                // Previous file is done, flush it (safety check)
                self.flush_current_file()?;
                
                if let Some(data) = msg.data {
                    if let Some(path_obj) = data.path {
                        // Store path as lossy string for display
                        self.current_file_path = Some(path_obj.as_string_lossy()?.into_owned());
                    }
                }
                Ok(())
            }
            RgKind::Match | RgKind::Context => {
                if let Some(data) = msg.data {
                    if let Some(lines) = data.lines {
                        let text = lines.as_string_lossy()?;
                        // Trim the trailing newline from the file content itself 
                        // so we control formatting
                        let content = text.trim_end_matches(&['\r', '\n'][..]);
                        
                        let line_num = data.line_number.unwrap_or(0);
                        
                        // Format: "  12 | code here"
                        self.match_buffer.push(format!("{:4} | {}", line_num, content));
                    }
                }
                Ok(())
            }
            RgKind::End => {
                // File processing complete, flush the buffer
                self.flush_current_file()?;
                Ok(())
            }
            RgKind::Summary => Ok(()), // Ignore summary stats for agents
        }
    }
}
