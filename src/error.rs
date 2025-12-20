use std::path::PathBuf;
use tempfile::PersistError;
use thiserror::Error;

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

    #[error("Transaction failed (partial application): {0}")]
    TransactionFailure(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Regex(_) => "E_REGEX",
            Error::Io(e) => match e.kind() {
                std::io::ErrorKind::NotFound => "E_NOT_FOUND",
                std::io::ErrorKind::PermissionDenied => "E_ACCES",
                std::io::ErrorKind::AlreadyExists => "E_EXIST",
                _ => "E_IO",
            },
            Error::Persist(_) => "E_PERSIST",
            Error::InvalidReplacement(_) => "E_INVALID_REPLACEMENT",
            Error::AmbiguousReplacement(_) => "E_AMBIGUOUS_REPLACEMENT",
            Error::Validation(_) => "E_VALIDATION",
            Error::NoInputSources => "E_NO_INPUT",
            Error::InputScopeConflict(_) => "E_INPUT_CONFLICT",
            Error::OutputModeConflict(_) => "E_OUTPUT_CONFLICT",
            Error::FeatureNotEnabled(_) => "E_FEATURE_DISABLED",
            Error::FailedJobs(_) => "E_FAILED_JOBS",
            Error::InvalidPath(_) => "E_INVALID_PATH",
            Error::TransactionFailure(_) => "E_TRANSACTION",
        }
    }
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
