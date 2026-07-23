use std::fs;
use std::path::PathBuf;

use dirs_next as dirs;

use crate::error::AppError;

use super::category::Category;
use super::item::{CleanupItem, PathAuthority};
use super::target::{CleanupTarget, DiscoveryOutcome, ScanScope};

pub struct BrewTarget;

impl BrewTarget {
    pub fn new() -> Self {
        Self
    }

    fn brew_paths() -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|home| {
                vec![home.join("Library/Caches/Homebrew"), home.join("Library/Logs/Homebrew")]
            })
            .unwrap_or_default()
    }
}

impl Default for BrewTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupTarget for BrewTarget {
    fn category(&self) -> Category {
        Category::Brew
    }

    fn discover(&self, _scope: &ScanScope) -> Result<DiscoveryOutcome, AppError> {
        let mut items = Vec::new();
        for path in Self::brew_paths() {
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    items.push(CleanupItem::from_path(
                        Category::Brew,
                        path.clone(),
                        PathAuthority::UserPath(path),
                    )?);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AppError::Traversal { path, reason: error.to_string() });
                }
            }
        }
        Ok(DiscoveryOutcome::Complete(items))
    }
}
