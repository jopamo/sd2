use anyhow::Result;
use std::io::{self, BufReader, Write};

mod rgjson;
use rgjson::{stream_rg_json_ndjson, RgKind, RgMessage, RgSink};

/// A sink that groups matches by file.
/// This prevents "interleaved" confusion for Agents and allows
/// constructing a cleaner context window.
struct BufferedAgentSink {
    stdout: io::StdoutLock<'static>,
    current_file_path: Option<String>,
    match_buffer: Vec<String>,
}

impl BufferedAgentSink {
    fn new() -> Self {
        // Leaking stdin/stdout is common/acceptable in CLI tools for 'static locks
        let stdout = Box::leak(Box::new(io::stdout())).lock();
        Self {
            stdout,
            current_file_path: None,
            match_buffer: Vec::new(),
        }
    }

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

fn main() -> Result<()> {
    let stdin = io::stdin();
    let reader = BufReader::new(stdin.lock());

    let mut sink = BufferedAgentSink::new();
    stream_rg_json_ndjson(reader, &mut sink)
}
