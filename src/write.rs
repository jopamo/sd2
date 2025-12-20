use crate::error::{Error, Result};
use crate::model::PermissionsMode;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

#[derive(Debug, Default)]
pub struct WriteOptions {
    pub no_follow_symlinks: bool,
    pub permissions: PermissionsMode,
}

/// A staged file write, ready to be committed.
pub struct StagedEntry {
    temp: NamedTempFile,
    target: PathBuf,
}

impl StagedEntry {
    /// Commit the staged file (atomic rename).
    pub fn commit(self) -> Result<()> {
        self.temp
            .persist(&self.target)
            .map_err(|e| Error::Io(e.error))?;
        Ok(())
    }
}

/// Prepare a file for writing (create temp, write content, copy perms).
pub fn stage_file(path: &Path, data: &[u8], options: &WriteOptions) -> Result<StagedEntry> {
    let target_path = resolve_symlink(path, options)?;

    // Write atomically using a temporary file in the same directory
    let parent = target_path
        .parent()
        .ok_or_else(|| Error::InvalidPath(target_path.to_path_buf()))?;

    let mut temp = NamedTempFile::new_in(parent)?;

    // Set permissions
    match options.permissions {
        PermissionsMode::Preserve => {
            if let Ok(metadata) = fs::metadata(&target_path) {
                temp.as_file().set_permissions(metadata.permissions()).ok();
            }
        }
        PermissionsMode::Fixed(mode) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let p = fs::Permissions::from_mode(mode);
                temp.as_file().set_permissions(p)?;
            }
        }
    }

    // Write data
    if !data.is_empty() {
        temp.write_all(data)?;
        temp.flush()?;
    }

    Ok(StagedEntry {
        temp,
        target: target_path,
    })
}

/// Write data to a file atomically.
/// Preserves file permissions and handles symbolic links according to options.
pub fn write_file(path: &Path, data: &[u8], options: &WriteOptions) -> Result<()> {
    let staged = stage_file(path, data, options)?;
    staged.commit()?;
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
