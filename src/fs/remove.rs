use std::ffi::{CString, OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, Stat, fstat, open, openat, statat, unlinkat,
};
use rustix::io::{Errno, dup};

use crate::error::AppError;
use crate::targets::item::{FileIdentity, FilesystemCandidate, ItemKind, PathAuthority};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalOutcome {
    Removed,
    Missing,
}

struct AuthorizedEntry {
    parent: OwnedFd,
    name: OsString,
    device: u64,
}

pub fn remove_candidate(candidate: &FilesystemCandidate) -> Result<RemovalOutcome, AppError> {
    let authorized = open_authorized_entry(candidate)?;
    remove_authorized_entry(candidate, &authorized)
}

fn open_authorized_entry(candidate: &FilesystemCandidate) -> Result<AuthorizedEntry, AppError> {
    match &candidate.authority {
        PathAuthority::LocalRoot { path: root, identity } => {
            if !candidate.path.starts_with(root) {
                return Err(revalidation_error(
                    &candidate.path,
                    format!("path is outside local root '{}'", root.display()),
                ));
            }

            if candidate.path == *root {
                let parent_path = root.parent().ok_or_else(|| {
                    revalidation_error(&candidate.path, "local root has no parent")
                })?;
                let parent = open_absolute_directory(parent_path, &candidate.path)?;
                let name = root
                    .file_name()
                    .ok_or_else(|| revalidation_error(&candidate.path, "local root has no name"))?
                    .to_os_string();
                return Ok(AuthorizedEntry { parent, name, device: identity.device });
            }

            let root_fd = open_absolute_directory(root, &candidate.path)?;
            validate_fd_identity(&root_fd, *identity, root)?;
            let relative = candidate
                .path
                .strip_prefix(root)
                .map_err(|error| revalidation_error(&candidate.path, error.to_string()))?;
            let (parent, name) =
                open_relative_parent(&root_fd, relative, identity.device, &candidate.path)?;
            Ok(AuthorizedEntry { parent, name, device: identity.device })
        }
        PathAuthority::UserPath { allowed, canonical_parent, parent_identity } => {
            if candidate.path != *allowed {
                return Err(revalidation_error(
                    &candidate.path,
                    format!("path does not equal allowlisted path '{}'", allowed.display()),
                ));
            }

            let parent = open_absolute_directory(canonical_parent, &candidate.path)?;
            validate_fd_identity(&parent, *parent_identity, canonical_parent)?;
            let name = allowed
                .file_name()
                .ok_or_else(|| revalidation_error(&candidate.path, "allowlisted path has no name"))?
                .to_os_string();
            Ok(AuthorizedEntry { parent, name, device: parent_identity.device })
        }
    }
}

fn open_absolute_directory(path: &Path, display: &Path) -> Result<OwnedFd, AppError> {
    if !path.is_absolute() {
        return Err(revalidation_error(
            display,
            format!("authority path '{}' is not absolute", path.display()),
        ));
    }

    let mut directory = open("/", directory_flags(), Mode::empty()).map_err(|error| {
        revalidation_error(display, format!("cannot open filesystem root: {error}"))
    })?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                directory = openat(&directory, name, directory_flags(), Mode::empty()).map_err(
                    |error| {
                        revalidation_error(
                            display,
                            format!(
                                "cannot open authority component '{}': {error}",
                                name.to_string_lossy()
                            ),
                        )
                    },
                )?;
            }
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) => {
                return Err(revalidation_error(
                    display,
                    format!("authority path '{}' is not normalized", path.display()),
                ));
            }
        }
    }
    Ok(directory)
}

fn open_relative_parent(
    root: &OwnedFd,
    relative: &Path,
    device: u64,
    display: &Path,
) -> Result<(OwnedFd, OsString), AppError> {
    let components = relative
        .components()
        .map(|component| match component {
            Component::Normal(name) => Ok(name.to_os_string()),
            _ => Err(revalidation_error(
                display,
                "candidate path is not normalized beneath its authority",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (name, parents) = components
        .split_last()
        .ok_or_else(|| revalidation_error(display, "candidate path has no entry name"))?;

    let mut directory = dup(root).map_err(|error| {
        revalidation_error(display, format!("cannot duplicate authority descriptor: {error}"))
    })?;
    for component in parents {
        let next =
            openat(&directory, component, directory_flags(), Mode::empty()).map_err(|error| {
                revalidation_error(
                    display,
                    format!(
                        "cannot open candidate parent '{}': {error}",
                        component.to_string_lossy()
                    ),
                )
            })?;
        let stat = fstat(&next).map_err(|error| {
            revalidation_error(display, format!("cannot inspect candidate parent: {error}"))
        })?;
        validate_device(&stat, device, display)?;
        directory = next;
    }

    Ok((directory, name.clone()))
}

fn remove_authorized_entry(
    candidate: &FilesystemCandidate,
    authorized: &AuthorizedEntry,
) -> Result<RemovalOutcome, AppError> {
    let stat = match statat(&authorized.parent, &authorized.name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Ok(RemovalOutcome::Missing),
        Err(error) => {
            return Err(removal_error(
                &candidate.path,
                format!("cannot inspect candidate: {error}"),
            ));
        }
    };

    validate_candidate_stat(candidate, &stat, authorized.device)?;

    match candidate.kind {
        ItemKind::File | ItemKind::Symlink => {
            revalidate_entry(
                &authorized.parent,
                &authorized.name,
                candidate.identity,
                authorized.device,
                &candidate.path,
            )?;
            unlink_entry(&authorized.parent, &authorized.name, AtFlags::empty(), &candidate.path)
        }
        ItemKind::Directory => {
            let directory = open_verified_directory(
                &authorized.parent,
                &authorized.name,
                &stat,
                authorized.device,
                &candidate.path,
            )?;
            validate_directory_tree(&directory, authorized.device, &candidate.path)?;
            remove_directory_contents(&directory, authorized.device, &candidate.path)?;
            revalidate_entry(
                &authorized.parent,
                &authorized.name,
                candidate.identity,
                authorized.device,
                &candidate.path,
            )?;
            unlink_entry(&authorized.parent, &authorized.name, AtFlags::REMOVEDIR, &candidate.path)
        }
    }
}

fn validate_directory_tree(
    directory: &OwnedFd,
    device: u64,
    display: &Path,
) -> Result<(), AppError> {
    for name in directory_entries(directory, display)? {
        let child_display = display.join(OsStr::from_bytes(name.to_bytes()));
        let stat = statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            revalidation_error(&child_display, format!("cannot inspect directory entry: {error}"))
        })?;
        validate_device(&stat, device, &child_display)?;

        if file_type(&stat) == FileType::Directory {
            let child = open_verified_directory(directory, &name, &stat, device, &child_display)?;
            validate_directory_tree(&child, device, &child_display)?;
        }
    }
    Ok(())
}

fn remove_directory_contents(
    directory: &OwnedFd,
    device: u64,
    display: &Path,
) -> Result<(), AppError> {
    for name in directory_entries(directory, display)? {
        let child_display = display.join(OsStr::from_bytes(name.to_bytes()));
        let stat = match statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(Errno::NOENT) => continue,
            Err(error) => {
                return Err(removal_error(
                    &child_display,
                    format!("cannot inspect directory entry: {error}"),
                ));
            }
        };
        validate_device(&stat, device, &child_display)?;
        let identity = stat_identity(&stat);

        if file_type(&stat) == FileType::Directory {
            let child = open_verified_directory(directory, &name, &stat, device, &child_display)?;
            remove_directory_contents(&child, device, &child_display)?;
            revalidate_entry(directory, &name, Some(identity), device, &child_display)?;
            match unlinkat(directory, &name, AtFlags::REMOVEDIR) {
                Ok(()) | Err(Errno::NOENT) => {}
                Err(error) => {
                    return Err(removal_error(
                        &child_display,
                        format!("cannot remove directory: {error}"),
                    ));
                }
            }
        } else {
            revalidate_entry(directory, &name, Some(identity), device, &child_display)?;
            match unlinkat(directory, &name, AtFlags::empty()) {
                Ok(()) | Err(Errno::NOENT) => {}
                Err(error) => {
                    return Err(removal_error(
                        &child_display,
                        format!("cannot remove entry: {error}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn directory_entries(directory: &OwnedFd, display: &Path) -> Result<Vec<CString>, AppError> {
    let entries = Dir::read_from(directory).map_err(|error| {
        removal_error(display, format!("cannot open directory stream: {error}"))
    })?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            removal_error(display, format!("cannot read directory entry: {error}"))
        })?;
        let name = entry.file_name();
        if name.to_bytes() != b"." && name.to_bytes() != b".." {
            names.push(name.to_owned());
        }
    }
    names.sort_by(|left, right| left.to_bytes().cmp(right.to_bytes()));
    Ok(names)
}

fn open_verified_directory<P: rustix::path::Arg>(
    parent: &OwnedFd,
    name: P,
    expected: &Stat,
    device: u64,
    display: &Path,
) -> Result<OwnedFd, AppError> {
    let directory = openat(parent, name, directory_flags(), Mode::empty()).map_err(|error| {
        revalidation_error(display, format!("cannot open directory entry: {error}"))
    })?;
    let opened = fstat(&directory).map_err(|error| {
        revalidation_error(display, format!("cannot inspect opened directory: {error}"))
    })?;
    validate_device(&opened, device, display)?;
    if stat_identity(&opened) != stat_identity(expected) {
        return Err(revalidation_error(display, "directory entry changed while opening"));
    }
    Ok(directory)
}

fn revalidate_entry<P: rustix::path::Arg>(
    parent: &OwnedFd,
    name: P,
    expected: Option<FileIdentity>,
    device: u64,
    display: &Path,
) -> Result<(), AppError> {
    let stat = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        revalidation_error(display, format!("entry changed before removal: {error}"))
    })?;
    validate_device(&stat, device, display)?;
    let Some(expected) = expected else {
        return Err(revalidation_error(display, "scan identity is unavailable"));
    };
    if stat_identity(&stat) != expected {
        return Err(revalidation_error(display, "filesystem object changed after validation"));
    }
    Ok(())
}

fn unlink_entry<P: rustix::path::Arg>(
    parent: &OwnedFd,
    name: P,
    flags: AtFlags,
    display: &Path,
) -> Result<RemovalOutcome, AppError> {
    match unlinkat(parent, name, flags) {
        Ok(()) => Ok(RemovalOutcome::Removed),
        Err(Errno::NOENT) => Ok(RemovalOutcome::Missing),
        Err(error) => Err(removal_error(display, format!("cannot remove candidate: {error}"))),
    }
}

fn validate_candidate_stat(
    candidate: &FilesystemCandidate,
    stat: &Stat,
    device: u64,
) -> Result<(), AppError> {
    validate_device(stat, device, &candidate.path)?;
    let actual_kind = item_kind(stat);
    if actual_kind != candidate.kind {
        return Err(revalidation_error(
            &candidate.path,
            format!("kind changed from {:?} to {:?}", candidate.kind, actual_kind),
        ));
    }
    let Some(expected) = candidate.identity else {
        return Err(revalidation_error(&candidate.path, "scan identity is unavailable"));
    };
    if stat_identity(stat) != expected {
        return Err(revalidation_error(
            &candidate.path,
            "filesystem object changed after scanning",
        ));
    }
    Ok(())
}

fn validate_fd_identity(
    fd: &OwnedFd,
    expected: FileIdentity,
    display: &Path,
) -> Result<(), AppError> {
    let stat = fstat(fd).map_err(|error| {
        revalidation_error(display, format!("cannot inspect authority: {error}"))
    })?;
    if stat_identity(&stat) != expected {
        return Err(revalidation_error(display, "authority directory changed after scanning"));
    }
    Ok(())
}

fn validate_device(stat: &Stat, expected: u64, display: &Path) -> Result<(), AppError> {
    if stat.st_dev as u64 != expected {
        return Err(revalidation_error(
            display,
            "entry crosses the authorized filesystem boundary",
        ));
    }
    Ok(())
}

fn stat_identity(stat: &Stat) -> FileIdentity {
    FileIdentity { device: stat.st_dev as u64, inode: stat.st_ino }
}

fn item_kind(stat: &Stat) -> ItemKind {
    match file_type(stat) {
        FileType::Directory => ItemKind::Directory,
        FileType::Symlink => ItemKind::Symlink,
        _ => ItemKind::File,
    }
}

fn file_type(stat: &Stat) -> FileType {
    FileType::from_raw_mode(stat.st_mode as _)
}

fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

fn revalidation_error(path: &Path, reason: impl Into<String>) -> AppError {
    AppError::Revalidation { path: path.to_path_buf(), reason: reason.into() }
}

fn removal_error(path: &Path, reason: impl Into<String>) -> AppError {
    AppError::Removal { path: path.to_path_buf(), reason: reason.into() }
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
            CleanupItem::local_authority(&canonical_root).expect("authority resolves"),
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

    #[test]
    fn opened_parent_descriptor_is_stable_after_path_replacement() {
        let root = TempDir::new().expect("root is created");
        let parent = root.child("workspace");
        let target = parent.child("node_modules");
        target.child("original.txt").write_str("content").expect("original target exists");
        let candidate = scanned_candidate(target.path(), root.path());
        let authorized = open_authorized_entry(&candidate).expect("parent is authorized");

        let moved = root.child("moved");
        std::fs::rename(parent.path(), moved.path()).expect("original parent is moved");
        let replacement = root.child("workspace/node_modules");
        replacement
            .child("replacement.txt")
            .write_str("content")
            .expect("replacement target exists");

        assert_eq!(
            remove_authorized_entry(&candidate, &authorized)
                .expect("descriptor-relative removal succeeds"),
            RemovalOutcome::Removed
        );
        moved.child("node_modules").assert(predicates::path::missing());
        replacement.child("replacement.txt").assert("content");
    }

    #[test]
    fn different_authority_device_fails_closed() {
        let root = TempDir::new().expect("root is created");
        let target = root.child("node_modules");
        target.create_dir_all().expect("target exists");
        let mut candidate = scanned_candidate(target.path(), root.path());
        match &mut candidate.authority {
            PathAuthority::LocalRoot { identity, .. } => {
                identity.device = identity.device.wrapping_add(1);
            }
            PathAuthority::UserPath { .. } => unreachable!("test uses local authority"),
        }

        assert!(matches!(remove_candidate(&candidate), Err(AppError::Revalidation { .. })));
        target.assert(predicates::path::is_dir());
    }
}
