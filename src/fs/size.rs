use std::io::ErrorKind;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::AppError;

/// Byte size of a file or directory tree.
///
/// A path that has vanished between discovery and measurement contributes 0. Other
/// traversal and metadata errors are surfaced rather than producing a partial estimate.
pub fn path_size(path: &Path) -> Result<u64, AppError> {
    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(AppError::Io(err)),
    };

    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0u64;
    for entry in WalkDir::new(path).into_iter() {
        let entry = entry.map_err(std::io::Error::other)?;

        if entry.file_type().is_file() {
            match entry.metadata() {
                Ok(metadata) => {
                    total = total.saturating_add(metadata.len());
                }
                Err(err)
                    if err.io_error().is_some_and(|error| error.kind() == ErrorKind::NotFound) => {}
                Err(err) => return Err(AppError::Io(std::io::Error::other(err))),
            }
        }
    }
    Ok(total)
}
