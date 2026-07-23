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

    for entry_result in WalkDir::new(path).contents_first(true).into_iter() {
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
            let entry_path = entry.path();
            match fs::remove_file(entry_path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(path_error("remove file or symbolic link", entry_path, source));
                }
            }
        } else if entry.file_type().is_dir() {
            let entry_path = entry.path();
            match fs::remove_dir(entry_path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {}
                Err(source) => {
                    return Err(path_error("remove directory", entry_path, source));
                }
            }
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

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;

    #[test]
    fn contents_first_removal_handles_wide_and_deep_trees() {
        let temp = TempDir::new().expect("temp directory is created");
        let root = temp.child("root");
        root.create_dir_all().expect("root exists");
        let mut deepest = root.path().to_path_buf();
        for depth in 0..64 {
            deepest.push(format!("level-{depth}"));
            fs::create_dir(&deepest).expect("deep directory exists");
            fs::write(deepest.join("cache.bin"), b"cache").expect("deep file exists");
        }
        for index in 0..256 {
            let directory = root.path().join(format!("wide-{index}"));
            fs::create_dir(&directory).expect("wide directory exists");
            fs::write(directory.join("cache.bin"), b"cache").expect("wide file exists");
        }

        let outcome = safe_remove_dir_all(root.path()).expect("tree removal succeeds");

        assert_eq!(outcome, RemovalStatus::Removed);
        root.assert(predicates::path::missing());
    }

    #[cfg(unix)]
    #[test]
    fn unsupported_entry_created_inside_tree_is_retained_explicitly() {
        use std::os::unix::net::UnixListener;

        let temp = TempDir::new().expect("temp directory is created");
        let root = temp.child("root");
        root.create_dir_all().expect("root exists");
        let socket = root.path().join("active.sock");
        let listener = UnixListener::bind(&socket).expect("socket exists");

        let outcome = safe_remove_dir_all(root.path()).expect("retained tree is not an I/O error");

        assert_eq!(outcome, RemovalStatus::Retained);
        assert!(socket.exists(), "unsupported active entry remains visible");
        drop(listener);
    }
}
