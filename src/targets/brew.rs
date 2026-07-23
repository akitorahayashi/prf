use std::fs;
use std::path::PathBuf;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::target::{CleanupTarget, DiscoveryOutcome, ScanScope};

pub struct BrewTarget;

impl BrewTarget {
    pub fn new() -> Self {
        Self
    }

    fn brew_paths() -> Result<Vec<PathBuf>, AppError> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(AppError::HomeDirectoryUnavailable)?;
        Ok(vec![home.join("Library/Caches/Homebrew"), home.join("Library/Logs/Homebrew")])
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
        for path in Self::brew_paths()? {
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    let authority = CleanupItem::user_authority(&path)?;
                    items.push(CleanupItem::from_path(Category::Brew, path.clone(), authority)?);
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
