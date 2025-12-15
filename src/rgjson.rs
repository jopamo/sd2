use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::BTreeMap;
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
    #[serde(default)]
    pub lines: Option<RgTextOrBytes>,
    #[serde(default)]
    pub line_number: Option<u64>,
    #[serde(default)]
    pub absolute_offset: Option<u64>,
    #[serde(default)]
    pub submatches: Vec<RgSubmatch>,
}

#[derive(Debug, Deserialize)]
pub struct RgSubmatch {
    #[serde(default)]
    pub m: Option<RgTextOrBytes>,
    #[serde(default)]
    pub start: u64,
    #[serde(default)]
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

/// A sink that groups matches by file using a map.
/// This handles interleaved output from threaded ripgrep correctly.
pub struct DeinterleavingSink {
    // Map from Path (OsString) to a list of messages for that path
    // We store the raw RgMessage (or a struct derived from it)
    pub events: BTreeMap<OsString, Vec<RgData>>,
}

impl DeinterleavingSink {
    pub fn new() -> Self {
        Self {
            events: BTreeMap::new(),
        }
    }
}

impl RgSink for DeinterleavingSink {
    fn handle(&mut self, msg: RgMessage) -> Result<()> {
        match msg.kind {
            RgKind::Match | RgKind::Context => {
                if let Some(data) = msg.data {
                     if let Some(ref path_obj) = data.path {
                         let path = path_obj.to_os_string()?;
                         self.events.entry(path).or_default().push(data);
                     }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

