use std::fs;
use std::io;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::AppError;

pub fn remove_file(path: &Path) -> Result<(), AppError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::Io(err)),
    }
}

pub fn safe_remove_dir_all(path: &Path) -> Result<(), AppError> {
    let mut files_to_remove = Vec::new();
    let mut dirs_to_remove = Vec::new();

    for entry_result in WalkDir::new(path).into_iter() {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error)
                if error
                    .io_error()
                    .is_some_and(|source| source.kind() == io::ErrorKind::NotFound) =>
            {
                continue;
            }
            Err(error) => return Err(AppError::Io(io::Error::other(error))),
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
    for (_, dir) in &dirs_to_remove {
        match fs::remove_dir(dir) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) if err.kind() == io::ErrorKind::DirectoryNotEmpty => {}
            Err(err) => return Err(AppError::Io(err)),
        }
    }

    Ok(())
}
