use std::path::PathBuf;

use super::category::Category;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    File,
    Directory,
}

/// What a `CleanupItem` reclaims: a filesystem path removal, or a Docker prune command.
///
/// Modeling the Docker prune as its own variant keeps command markers out of the
/// filesystem deletion path by construction — `fs::remove::remove_item` only ever
/// receives the `Path` variant's fields.
#[derive(Debug, Clone)]
pub enum CleanupAction {
    Path { path: PathBuf, kind: ItemKind },
    DockerPrune,
}

#[derive(Debug, Clone)]
pub struct CleanupItem {
    pub category: Category,
    pub action: CleanupAction,
    pub size: u64,
}

impl CleanupItem {
    pub fn directory(category: Category, path: PathBuf, size: u64) -> Self {
        Self { category, action: CleanupAction::Path { path, kind: ItemKind::Directory }, size }
    }

    pub fn file(category: Category, path: PathBuf, size: u64) -> Self {
        Self { category, action: CleanupAction::Path { path, kind: ItemKind::File }, size }
    }

    pub fn docker_prune(size: u64) -> Self {
        Self { category: Category::Docker, action: CleanupAction::DockerPrune, size }
    }

    pub fn is_zero(&self) -> bool {
        self.size == 0
    }
}
