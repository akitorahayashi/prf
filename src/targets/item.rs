use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::AppError;

use super::category::Category;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathAuthority {
    LocalRoot { path: PathBuf, identity: FileIdentity },
    UserPath { allowed: PathBuf, canonical_parent: PathBuf, parent_identity: FileIdentity },
}

impl PathAuthority {
    pub fn device(&self) -> u64 {
        match self {
            PathAuthority::LocalRoot { identity, .. }
            | PathAuthority::UserPath { parent_identity: identity, .. } => identity.device,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemCandidate {
    pub path: PathBuf,
    pub kind: ItemKind,
    pub authority: PathAuthority,
    pub identity: Option<FileIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalAction {
    DockerPrune,
}

impl ExternalAction {
    pub fn description(self) -> &'static str {
        match self {
            ExternalAction::DockerPrune => "docker system prune --all --force --volumes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupAction {
    Filesystem(FilesystemCandidate),
    External(ExternalAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    pub identity: FileIdentity,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub struct CleanupItem {
    pub categories: BTreeSet<Category>,
    pub action: CleanupAction,
    pub size: u64,
    allocations: Vec<Allocation>,
}

impl CleanupItem {
    pub fn filesystem(
        category: Category,
        path: PathBuf,
        kind: ItemKind,
        authority: PathAuthority,
    ) -> Self {
        Self {
            categories: BTreeSet::from([category]),
            action: CleanupAction::Filesystem(FilesystemCandidate {
                path,
                kind,
                authority,
                identity: None,
            }),
            size: 0,
            allocations: Vec::new(),
        }
    }

    pub fn directory(category: Category, path: PathBuf, authority: PathAuthority) -> Self {
        Self::filesystem(category, path, ItemKind::Directory, authority)
    }

    pub fn from_path(
        category: Category,
        path: PathBuf,
        authority: PathAuthority,
    ) -> Result<Self, AppError> {
        let metadata = fs::symlink_metadata(&path).map_err(|error| AppError::Traversal {
            path: path.clone(),
            reason: error.to_string(),
        })?;
        let kind = if metadata.file_type().is_symlink() {
            ItemKind::Symlink
        } else if metadata.is_dir() {
            ItemKind::Directory
        } else {
            ItemKind::File
        };
        let mut item = Self::filesystem(category, path, kind, authority);
        item.set_identity(FileIdentity { device: metadata.dev(), inode: metadata.ino() });
        Ok(item)
    }

    pub fn user_authority(path: &Path) -> Result<PathAuthority, AppError> {
        let parent = path.parent().ok_or_else(|| AppError::Traversal {
            path: path.to_path_buf(),
            reason: "allowlisted path has no parent".to_string(),
        })?;
        let canonical_parent = fs::canonicalize(parent).map_err(|error| AppError::Traversal {
            path: parent.to_path_buf(),
            reason: error.to_string(),
        })?;
        let metadata = fs::symlink_metadata(&canonical_parent).map_err(|error| {
            AppError::Traversal { path: canonical_parent.clone(), reason: error.to_string() }
        })?;
        Ok(PathAuthority::UserPath {
            allowed: path.to_path_buf(),
            canonical_parent,
            parent_identity: file_identity(&metadata),
        })
    }

    pub fn local_authority(path: &Path) -> Result<PathAuthority, AppError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| AppError::Traversal {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
        if !metadata.is_dir() {
            return Err(AppError::Traversal {
                path: path.to_path_buf(),
                reason: "local authority is not a directory".to_string(),
            });
        }
        Ok(PathAuthority::LocalRoot {
            path: path.to_path_buf(),
            identity: file_identity(&metadata),
        })
    }

    pub fn docker_prune(size: u64) -> Self {
        Self {
            categories: BTreeSet::from([Category::Docker]),
            action: CleanupAction::External(ExternalAction::DockerPrune),
            size,
            allocations: Vec::new(),
        }
    }

    pub fn categories(&self) -> impl Iterator<Item = Category> + '_ {
        self.categories.iter().copied()
    }

    pub fn has_category(&self, category: Category) -> bool {
        self.categories.contains(&category)
    }

    pub fn retain_categories(&mut self, selected: &[Category]) {
        self.categories.retain(|category| selected.contains(category));
    }

    pub fn merge_categories(&mut self, other: &Self) {
        self.categories.extend(other.categories.iter().copied());
    }

    pub fn filesystem_candidate(&self) -> Option<&FilesystemCandidate> {
        match &self.action {
            CleanupAction::Filesystem(candidate) => Some(candidate),
            CleanupAction::External(_) => None,
        }
    }

    pub fn external_action(&self) -> Option<ExternalAction> {
        match self.action {
            CleanupAction::Filesystem(_) => None,
            CleanupAction::External(action) => Some(action),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.filesystem_candidate().map(|candidate| candidate.path.as_path())
    }

    pub fn set_allocations(&mut self, allocations: Vec<Allocation>) {
        self.size = allocations
            .iter()
            .fold(0u64, |total, allocation| total.saturating_add(allocation.bytes));
        self.allocations = allocations;
    }

    pub fn set_identity(&mut self, identity: FileIdentity) {
        if let CleanupAction::Filesystem(candidate) = &mut self.action {
            candidate.identity = Some(identity);
        }
    }

    pub fn allocations(&self) -> &[Allocation] {
        &self.allocations
    }
}

fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    FileIdentity { device: metadata.dev(), inode: metadata.ino() }
}
