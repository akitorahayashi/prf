use std::fs;
use std::io;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalStatus {
    Removed,
    AlreadyAbsent,
    Retained,
}

pub fn remove_file(path: &Path) -> Result<RemovalStatus, AppError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(RemovalStatus::Removed),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(RemovalStatus::AlreadyAbsent),
        Err(source) => Err(path_error("remove file or symbolic link", path, source)),
    }
}

pub fn safe_remove_dir_all(path: &Path) -> Result<RemovalStatus, AppError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(path_error(
                "inspect directory",
                path,
                io::Error::new(io::ErrorKind::InvalidInput, "entry is not a directory"),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemovalStatus::AlreadyAbsent);
        }
        Err(source) => return Err(path_error("inspect directory", path, source)),
    }

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
            Err(error) => {
                let failed_path = error.path().unwrap_or(path);
                return Err(path_error(
                    "traverse directory",
                    failed_path,
                    io::Error::other(error.to_string()),
                ));
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
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(path_error("remove file or symbolic link", file, source)),
        }
    }

    dirs_to_remove.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
    for (_, dir) in &dirs_to_remove {
        match fs::remove_dir(dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {}
            Err(source) => return Err(path_error("remove directory", dir, source)),
        }
    }

    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(RemovalStatus::Removed),
        Ok(_) => Ok(RemovalStatus::Retained),
        Err(source) => Err(path_error("inspect cleanup result", path, source)),
    }
}

fn path_error(operation: &'static str, path: &Path, source: io::Error) -> AppError {
    AppError::PathOperation { operation, path: path.to_path_buf(), source }
}
