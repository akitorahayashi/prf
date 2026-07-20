use std::fs;
use std::io;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::AppError;
use crate::targets::item::ItemKind;

/// Removes a filesystem item, returning the number of directories that could not be
/// removed because they were not empty after the cleanup pass (a non-fatal skip that
/// callers surface rather than swallow).
pub fn remove_item(path: &Path, kind: ItemKind, verbose: bool) -> Result<usize, AppError> {
    match kind {
        ItemKind::Directory => safe_remove_dir_all(path, verbose),
        ItemKind::File => match fs::remove_file(path) {
            Ok(()) => Ok(0),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(0),
            Err(err) => Err(AppError::Io(err)),
        },
    }
}

pub fn safe_remove_dir_all(path: &Path, verbose: bool) -> Result<usize, AppError> {
    let mut files_to_remove = Vec::new();
    let mut dirs_to_remove = Vec::new();

    for entry_result in WalkDir::new(path).into_iter() {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(err) => {
                if verbose {
                    eprintln!("Skipping due to error: {}", err);
                }
                continue;
            }
        };

        if entry.file_type().is_file() || entry.file_type().is_symlink() {
            files_to_remove.push(entry.into_path());
        } else if entry.file_type().is_dir() {
            dirs_to_remove.push((entry.depth(), entry.into_path()));
        }
    }

    for file in &files_to_remove {
        match fs::remove_file(file) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(AppError::Io(err)),
        }
    }

    dirs_to_remove.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
    let mut skipped = 0usize;
    for (_, dir) in &dirs_to_remove {
        match fs::remove_dir(dir) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) if err.kind() == io::ErrorKind::DirectoryNotEmpty => {
                skipped += 1;
                if verbose {
                    eprintln!(
                        "Directory not empty after cleanup pass, skipping: {}",
                        dir.display()
                    );
                }
            }
            Err(err) => return Err(AppError::Io(err)),
        }
    }

    Ok(skipped)
}
