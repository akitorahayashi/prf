use std::io::ErrorKind;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::AppError;

/// Byte size of a file or directory tree.
///
/// A path that has vanished between discovery and measurement (`NotFound`, a benign race
/// with a concurrent build) contributes 0, with a verbose-mode note; sizing is
/// non-destructive so a missing path is tolerated. Any other error (permission denied,
/// I/O failure) is surfaced rather than silently reported as 0.
pub fn path_size(path: &Path, verbose: bool) -> Result<u64, AppError> {
    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            if verbose {
                eprintln!("Skipping {}: {}", path.display(), err);
            }
            return Ok(0);
        }
        Err(err) => return Err(AppError::Io(err)),
    };

    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0u64;
    for entry in WalkDir::new(path).into_iter() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                if verbose {
                    eprintln!("Skipping {:?}: {}", err.path(), err);
                }
                continue;
            }
        };

        if entry.file_type().is_file() {
            match entry.metadata() {
                Ok(metadata) => {
                    total = total.saturating_add(metadata.len());
                }
                Err(err) => {
                    if verbose {
                        eprintln!("Skipping {}: {}", entry.path().display(), err);
                    }
                }
            }
        }
    }
    Ok(total)
}
