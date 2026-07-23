use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::AppError;
use crate::targets::item::{FileIdentity, FilesystemCandidate, ItemKind, PathAuthority};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalOutcome {
    Removed,
    Missing,
}

pub fn remove_candidate(candidate: &FilesystemCandidate) -> Result<RemovalOutcome, AppError> {
    let metadata = match fs::symlink_metadata(&candidate.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemovalOutcome::Missing);
        }
        Err(error) => return Err(removal_error(&candidate.path, error)),
    };

    validate_authority(candidate)?;
    validate_kind(candidate, &metadata)?;
    validate_identity(candidate, &metadata)?;

    match candidate.kind {
        ItemKind::File | ItemKind::Symlink => remove_file(&candidate.path),
        ItemKind::Directory => remove_directory(&candidate.path),
    }
}

fn validate_authority(candidate: &FilesystemCandidate) -> Result<(), AppError> {
    let path = &candidate.path;
    let invalid = |reason: String| AppError::Revalidation { path: path.clone(), reason };

    match &candidate.authority {
        PathAuthority::LocalRoot(root) => {
            if !path.starts_with(root) {
                return Err(invalid(format!("path is outside local root '{}'", root.display())));
            }

            if path == root {
                let canonical =
                    fs::canonicalize(path).map_err(|error| invalid(error.to_string()))?;
                if canonical != *root {
                    return Err(invalid(format!("root now resolves to '{}'", canonical.display())));
                }
            } else {
                let parent =
                    path.parent().ok_or_else(|| invalid("candidate has no parent".to_string()))?;
                let canonical_parent =
                    fs::canonicalize(parent).map_err(|error| invalid(error.to_string()))?;
                if !canonical_parent.starts_with(root) {
                    return Err(invalid(format!(
                        "parent now resolves outside local root to '{}'",
                        canonical_parent.display()
                    )));
                }
            }
        }
        PathAuthority::UserPath { allowed, canonical_parent } => {
            if path != allowed {
                return Err(invalid(format!(
                    "path does not equal allowlisted path '{}'",
                    allowed.display()
                )));
            }
            let parent =
                path.parent().ok_or_else(|| invalid("candidate has no parent".to_string()))?;
            let current_parent =
                fs::canonicalize(parent).map_err(|error| invalid(error.to_string()))?;
            if current_parent != *canonical_parent {
                return Err(invalid(format!(
                    "allowlisted parent changed from '{}' to '{}'",
                    canonical_parent.display(),
                    current_parent.display()
                )));
            }
        }
    }

    Ok(())
}

fn validate_kind(candidate: &FilesystemCandidate, metadata: &fs::Metadata) -> Result<(), AppError> {
    let actual = if metadata.file_type().is_symlink() {
        ItemKind::Symlink
    } else if metadata.is_dir() {
        ItemKind::Directory
    } else {
        ItemKind::File
    };
    if actual != candidate.kind {
        return Err(AppError::Revalidation {
            path: candidate.path.clone(),
            reason: format!("kind changed from {:?} to {:?}", candidate.kind, actual),
        });
    }
    Ok(())
}

fn validate_identity(
    candidate: &FilesystemCandidate,
    metadata: &fs::Metadata,
) -> Result<(), AppError> {
    let actual = FileIdentity { device: metadata.dev(), inode: metadata.ino() };
    let Some(expected) = candidate.identity else {
        return Err(AppError::Revalidation {
            path: candidate.path.clone(),
            reason: "scan identity is unavailable".to_string(),
        });
    };
    if actual != expected {
        return Err(AppError::Revalidation {
            path: candidate.path.clone(),
            reason: "filesystem object changed after scanning".to_string(),
        });
    }
    Ok(())
}

fn remove_file(path: &Path) -> Result<RemovalOutcome, AppError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(RemovalOutcome::Removed),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(RemovalOutcome::Missing),
        Err(error) => Err(removal_error(path, error)),
    }
}

fn remove_directory(path: &Path) -> Result<RemovalOutcome, AppError> {
    let mut files = Vec::new();
    let mut directories = Vec::new();

    for result in WalkDir::new(path).follow_links(false) {
        let entry = result.map_err(|error| AppError::Removal {
            path: error.path().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf()),
            reason: error.to_string(),
        })?;
        if entry.file_type().is_file() || entry.file_type().is_symlink() {
            files.push(entry.into_path());
        } else if entry.file_type().is_dir() {
            directories.push((entry.depth(), entry.into_path()));
        }
    }

    for file in files {
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(removal_error(&file, error)),
        }
    }

    directories.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
    let mut root_removed = false;
    for (_, directory) in directories {
        match fs::remove_dir(&directory) {
            Ok(()) => {
                if directory == path {
                    root_removed = true;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(removal_error(&directory, error)),
        }
    }

    Ok(if root_removed { RemovalOutcome::Removed } else { RemovalOutcome::Missing })
}

fn removal_error(path: &Path, error: io::Error) -> AppError {
    AppError::Removal { path: path.to_path_buf(), reason: error.to_string() }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;
    use crate::targets::category::Category;
    use crate::targets::item::CleanupItem;

    fn scanned_candidate(path: &Path, root: &Path) -> FilesystemCandidate {
        let canonical_path = std::fs::canonicalize(path).expect("candidate canonicalizes");
        let canonical_root = std::fs::canonicalize(root).expect("root canonicalizes");
        CleanupItem::from_path(
            Category::Nodejs,
            canonical_path,
            PathAuthority::LocalRoot(canonical_root),
        )
        .expect("candidate is scanned")
        .filesystem_candidate()
        .expect("candidate is a filesystem path")
        .clone()
    }

    #[test]
    fn removal_does_not_follow_symbolic_links_inside_directory() {
        use std::os::unix::fs::symlink;

        let root = TempDir::new().expect("root is created");
        let outside = TempDir::new().expect("outside root is created");
        outside.child("preserved.txt").write_str("content").expect("outside file exists");
        let target = root.child("node_modules");
        target.create_dir_all().expect("target exists");
        symlink(outside.path(), target.child("linked").path()).expect("symlink exists");
        let candidate = scanned_candidate(target.path(), root.path());

        assert_eq!(
            remove_candidate(&candidate).expect("removal succeeds"),
            RemovalOutcome::Removed
        );
        outside.child("preserved.txt").assert("content");
    }

    #[test]
    fn directory_replaced_by_symlink_fails_closed() {
        use std::os::unix::fs::symlink;

        let root = TempDir::new().expect("root is created");
        let outside = TempDir::new().expect("outside root is created");
        outside.child("preserved.txt").write_str("content").expect("outside file exists");
        let target = root.child("node_modules");
        target.create_dir_all().expect("target exists");
        let candidate = scanned_candidate(target.path(), root.path());

        std::fs::remove_dir(target.path()).expect("original target is removed");
        symlink(outside.path(), target.path()).expect("replacement symlink exists");

        assert!(matches!(remove_candidate(&candidate), Err(AppError::Revalidation { .. })));
        outside.child("preserved.txt").assert("content");
    }

    #[test]
    fn missing_candidate_is_a_skip() {
        let root = TempDir::new().expect("root is created");
        let target = root.child("node_modules");
        target.create_dir_all().expect("target exists");
        let candidate = scanned_candidate(target.path(), root.path());
        std::fs::remove_dir(target.path()).expect("target is removed");

        assert_eq!(
            remove_candidate(&candidate).expect("missing candidate is accepted"),
            RemovalOutcome::Missing
        );
    }
}
