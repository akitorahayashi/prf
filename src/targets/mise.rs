use std::path::PathBuf;

use dirs_next as dirs;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::target::{CleanupTarget, ScanScope};

pub struct MiseTarget {
    home_override: Option<PathBuf>,
}

impl MiseTarget {
    pub fn new() -> Self {
        Self { home_override: None }
    }

    #[cfg(test)]
    fn with_home(home: PathBuf) -> Self {
        Self { home_override: Some(home) }
    }

    fn mise_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(home) = self.home_override.clone().or_else(dirs::home_dir) {
            // ~/.cache/mise
            paths.push(home.join(".cache/mise"));
            // ~/.local/share/mise/downloads
            paths.push(home.join(".local/share/mise/downloads"));
            // ~/.local/share/mise/tmp
            paths.push(home.join(".local/share/mise/tmp"));
            // ~/Library/Caches/mise (macOS specific, but safe to include)
            paths.push(home.join("Library/Caches/mise"));
        }
        paths
    }
}

impl Default for MiseTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupTarget for MiseTarget {
    fn category(&self) -> Category {
        Category::Mise
    }

    fn discover(&self, _scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        let mut items = Vec::new();
        for path in self.mise_paths() {
            if path.is_dir() {
                items.push(CleanupItem::directory(Category::Mise, path, 0));
            }
        }
        Ok(items)
    }

    fn list(&self, _scope: &ScanScope) -> Result<Vec<String>, AppError> {
        let mut targets = Vec::new();
        for path in self.mise_paths() {
            if path.exists() {
                targets.push(format!("{} (exists)", path.display()));
            }
        }
        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::target::ScanScope;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_discover_mise_directories() {
        let temp = tempdir().unwrap();
        let home = temp.path().to_path_buf();

        let cache_mise = home.join(".cache/mise");
        let local_mise_downloads = home.join(".local/share/mise/downloads");

        fs::create_dir_all(&cache_mise).unwrap();
        fs::create_dir_all(&local_mise_downloads).unwrap();

        let target = MiseTarget::with_home(home);
        let scope = ScanScope::new(vec![], false, false);
        let items = target.discover(&scope).unwrap();

        assert_eq!(items.len(), 2);
        let paths: Vec<PathBuf> = items.into_iter().map(|i| i.path().to_path_buf()).collect();
        assert!(paths.contains(&cache_mise));
        assert!(paths.contains(&local_mise_downloads));
    }
}
