use crate::error::{Error, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Options for file writing.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Create backup before replacing. If Some(ext), use that extension.
    pub backup: Option<Option<String>>,
    /// Do not follow symbolic links (operate on symlink itself).
    pub no_follow_symlinks: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            backup: None,
            no_follow_symlinks: false,
        }
    }
}

/// Write data to a file atomically with optional backup.
/// Preserves file permissions and handles symbolic links according to options.
pub fn write_file(path: &Path, data: &[u8], options: &WriteOptions) -> Result<()> {
    let target_path = resolve_symlink(path, options)?;

    // Create backup if requested
    if let Some(backup_ext) = &options.backup {
        create_backup(&target_path, backup_ext)?;
    }

    // Write atomically using a temporary file in the same directory
    let parent = target_path.parent()
        .ok_or_else(|| Error::InvalidPath(target_path.to_path_buf()))?;

    let mut temp = NamedTempFile::new_in(parent)?;

    // Preserve permissions if the target file exists
    if let Ok(metadata) = fs::metadata(&target_path) {
        temp.as_file().set_permissions(metadata.permissions()).ok();
    }

    // Write data
    if !data.is_empty() {
        temp.write_all(data)?;
        temp.flush()?;
    }

    // Atomically replace the target file
    temp.persist(&target_path)?;
    Ok(())
}

/// Resolve symbolic links according to options.
fn resolve_symlink(path: &Path, options: &WriteOptions) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        if options.no_follow_symlinks {
            // Operate on the symlink itself
            return Ok(path.to_path_buf());
        } else {
            // Follow symlink (default)
            let target = fs::canonicalize(path)?;
            return Ok(target);
        }
    }
    // Not a symlink
    Ok(path.to_path_buf())
}

/// Create a backup of the file with the given extension.
/// If extension is None, use ".bak".
fn create_backup(path: &Path, extension: &Option<String>) -> Result<()> {
    let backup_path = match extension {
        Some(ext) => path.with_extension(format!("{}.{}", path.extension().unwrap_or_default().to_string_lossy(), ext)),
        None => path.with_extension("bak"),
    };
    fs::copy(path, backup_path)?;
    Ok(())
}