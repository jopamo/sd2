use thiserror::Error;
use std::path::PathBuf;
use tempfile::PersistError;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum Error {
    #[error("Invalid regex: {0}")]
    Regex(#[from] regex::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to persist temporary file: {0}")]
    Persist(#[from] PersistError),

    #[error("Invalid replacement pattern: {0}")]
    InvalidReplacement(String),

    #[error("Ambiguous replacement pattern: {0}")]
    AmbiguousReplacement(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("No input sources specified")]
    NoInputSources,

    #[error("Input scope conflict: {0}")]
    InputScopeConflict(String),

    #[error("Output mode conflict: {0}")]
    OutputModeConflict(String),

    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(&'static str),

    #[error("Failed to process files: {0}")]
    FailedJobs(FailedJobs),

    #[error("Invalid path: {0:?}")]
    InvalidPath(PathBuf),
}

#[derive(Debug)]
pub struct FailedJobs(pub Vec<(PathBuf, Error)>);

impl std::fmt::Display for FailedJobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} files failed", self.0.len())?;
        for (path, error) in &self.0 {
            write!(f, "\n  {}: {}", path.display(), error)?;
        }
        Ok(())
    }
}

pub type Result<T> = std::result::Result<T, Error>;