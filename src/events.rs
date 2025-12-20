use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    RunStart(RunStart),
    File(FileEvent),
    RunEnd(RunEnd),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStart {
    pub schema_version: String,
    pub tool_version: String,
    pub mode: String, // "cli" or "apply"
    pub input_mode: String, // "args", "stdin-paths", "stdin-text", "rg-json", "files0", "manifest"
    pub transaction_mode: String, // "all" or "file"
    pub dry_run: bool,
    pub validate_only: bool,
    pub no_write: bool,
    pub policies: Policies,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policies {
    pub require_match: bool,
    pub expect: Option<usize>,
    pub fail_on_change: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileEvent {
    Success {
        path: PathBuf,
        modified: bool,
        replacements: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        diff: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        generated_content: Option<String>,
        #[serde(default)]
        diff_is_binary: bool,
        #[serde(default)]
        is_virtual: bool,
    },
    Skipped {
        path: PathBuf,
        reason: SkipReason,
    },
    Error {
        path: PathBuf,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    Binary,
    Symlink,
    GlobExclude,
    #[serde(untagged)]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEnd {
    pub total_files: usize,
    pub total_processed: usize,
    pub total_modified: usize,
    pub total_replacements: usize,
    pub has_errors: bool,
    pub policy_violation: Option<String>,
    pub committed: bool,
    pub duration_ms: u64,
    pub exit_code: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_reason_serialization() {
        // Standard variants
        let r = SkipReason::Binary;
        assert_eq!(serde_json::to_string(&r).unwrap(), "\"binary\"");

        let r = SkipReason::Symlink;
        assert_eq!(serde_json::to_string(&r).unwrap(), "\"symlink\"");

        // Other variant (untagged)
        let r = SkipReason::Other("custom reason".into());
        assert_eq!(serde_json::to_string(&r).unwrap(), "\"custom reason\"");
        
        // Edge case: string that matches a variant name
        // Because of the order and 'untagged', deserialization might prefer the explicit variant if it matches?
        // But for serialization, it should just be the string.
        let r = SkipReason::Other("binary".into());
        assert_eq!(serde_json::to_string(&r).unwrap(), "\"binary\"");
    }

    #[test]
    fn test_skip_reason_deserialization() {
        let r: SkipReason = serde_json::from_str("\"binary\"").unwrap();
        assert_eq!(r, SkipReason::Binary);

        let r: SkipReason = serde_json::from_str("\"glob_exclude\"").unwrap();
        assert_eq!(r, SkipReason::GlobExclude);

        let r: SkipReason = serde_json::from_str("\"custom\"").unwrap();
        assert_eq!(r, SkipReason::Other("custom".into()));
    }
}
